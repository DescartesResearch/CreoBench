use std::sync::Arc;

use crate::orchestrator::config::LoadTestConfig;
use crate::orchestrator::io::OutputDir;
use crate::orchestrator::persist::{
    ConsolePersister, IntervalCsvPersister, PersistError, Persister, PhaseClassifier,
    TransactionCsvPersister, interval, transactions, writer_loop,
};
use crate::orchestrator::phases::collect::CollectError;
use crate::orchestrator::phases::{ConnectHandle, GeneratorHandle, collect_reports};
use crate::orchestrator::{Error, Result, distribute_profile};
use crate::tracker::IntervalReport;
use crate::transaction::LoadGeneratorId;
use crate::wire::command::Command;
use crate::wire::configure::{ConfigMessage, LoadGeneratorConfig};
use crate::wire::{LoadProfile, ServiceRegistry, Warmup};

pub struct LoadTestRunner {
    config: LoadTestConfig,
    handles: Vec<ConnectHandle>,
}

impl LoadTestRunner {
    pub fn new(config: LoadTestConfig) -> Self {
        let handles = config
            .generators
            .iter()
            .map(|g| ConnectHandle::new(g.to_string().into()))
            .collect();
        Self { config, handles }
    }

    pub async fn run(self) -> Result<()> {
        let output_dir = OutputDir::new(
            self.config.output_dir.clone(),
            self.config.overwrite_outputs,
        )
        .await?;

        let load_generator_ids = (0..self.handles.len())
            .map(u8::try_from)
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|_| Error::LoadGeneratorOverflow(self.handles.len()))?
            .into_iter()
            .map(LoadGeneratorId::new);

        let split_profiles = distribute_profile(&self.config.profile, self.handles.len());

        // Phase 1: connect all (collect all results)
        tracing::info!("Connecting to all load generator instances...");
        let mut tasks = Vec::with_capacity(self.handles.len());
        for handle in self.handles {
            tasks.push(tokio::spawn(async move { handle.connect().await }));
        }

        let mut connected = Vec::with_capacity(tasks.len());
        let mut first_error = None;

        for task in tasks {
            match task.await.expect("task panicked") {
                Ok(c) => {
                    tracing::debug!("Connected to `{}`.", c.addr());
                    connected.push(c);
                }
                Err(e) => {
                    tracing::error!("Failed to connect: {}", e);
                    first_error.get_or_insert(Error::Connect(e));
                }
            }
        }

        if let Some(err) = first_error {
            if !connected.is_empty() {
                tracing::info!(
                    "Some generators failed to connect; sending abort to connected instances..."
                );
                for handle in connected {
                    if let Err(err) = handle.send_abort().await {
                        tracing::error!("Failed to send abort signal: {err}")
                    };
                }
            }
            return Err(err);
        }

        tracing::info!("Connected to all load generator instances!");

        // Phase 2: configure all (barrier)
        assert_eq!(connected.len(), load_generator_ids.len());
        assert_eq!(connected.len(), split_profiles.len());
        tracing::info!("Sending load-generator configuration to load generator instances...");
        let mut tasks = Vec::with_capacity(connected.len());
        for (id, (handle, profile)) in load_generator_ids
            .into_iter()
            .zip(connected.into_iter().zip(&split_profiles))
        {
            let config_msg = ConfigMessage::Config(LoadGeneratorConfig {
                profile: LoadProfile::from(profile.clone()),
                script: Arc::clone(&self.config.script),
                registry: ServiceRegistry::from(self.config.registry.clone()),
                warmup: Warmup::from(self.config.warmup.clone()),
                virtual_user_count: self.config.virtual_user_count,
                seed: self.config.seed,
                timeout_ms: self.config.timeout_ms,
                load_generator_id: id,
            });
            tasks.push(tokio::spawn(
                async move { handle.configure(&config_msg).await },
            ));
        }

        let mut configured = Vec::with_capacity(tasks.len());
        let mut all_ready = true;
        for (id, task) in tasks.into_iter().enumerate() {
            match task.await.expect("task panicked") {
                Ok(start_handle) => {
                    tracing::debug!("Loadgenerator `{}` is ready.", id);
                    configured.push(start_handle);
                }
                Err(err) => {
                    tracing::error!("Failed to setup Loadgenerator `{}`: {}", id, err);
                    all_ready = false;
                }
            }
        }

        if !all_ready {
            tracing::info!(
                "Some generators failed to setup; sending Abort signal to ready instances..."
            );
            for handle in configured {
                let _rx = handle.start(Command::Abort).await?;
            }
            return Err(Error::Abort);
        }

        // Phase 3: start all (barrier)
        let mut tasks = Vec::with_capacity(configured.len());
        for handle in configured {
            tasks.push(tokio::spawn(
                async move { handle.start(Command::Start).await },
            ));
        }
        let mut handles: Vec<GeneratorHandle> = Vec::with_capacity(tasks.len());
        for task in tasks {
            handles.push(task.await.expect("task panicked")?);
        }

        // Phase 4: collect results (round-robin) and persist them
        let (report_tx, report_rx) = tokio::sync::mpsc::channel(2);
        let (error_tx, mut error_rx) = tokio::sync::oneshot::channel();
        let interval_persister = create_or_abort(
            &handles,
            open_persister(&output_dir, interval::FILE_NAME, IntervalCsvPersister::new),
        )
        .await?;
        let transactions_persister = create_or_abort(
            &handles,
            open_persister(
                &output_dir,
                transactions::FILE_NAME,
                TransactionCsvPersister::new,
            ),
        )
        .await?;
        let persisters: Vec<Box<dyn Persister>> = vec![
            Box::new(ConsolePersister::new(
                std::io::stderr(),
                self.config.warmup.duration,
                self.config.warmup.pause,
            )),
            Box::new(interval_persister),
            Box::new(transactions_persister),
        ];
        let writer_task = tokio::spawn(writer_loop(
            report_rx,
            PhaseClassifier::new(self.config.warmup.duration, self.config.warmup.pause),
            persisters,
            error_tx,
        ));

        let collect_outcome = collect_and_persist(&mut handles, &report_tx, &mut error_rx).await;
        drop(report_tx);
        writer_task.await.expect("writer task panicked");

        match collect_outcome {
            CollectOutcome::Exhausted => match error_rx.try_recv() {
                Ok(err) => Err(Error::Persist(err)),
                Err(_) => {
                    tracing::info!("Load test finished.");
                    Ok(())
                }
            },
            CollectOutcome::CollectFailed(err) => Err(Error::Collect(err)),
            CollectOutcome::PersistFailed(err) => {
                tracing::info!(
                    "A persister failed; sending Abort signal to ready load generator instances..."
                );
                for handle in &handles {
                    if let Err(err) = handle.send_abort().await {
                        tracing::error!("Failed to send abort signal: {err}");
                    }
                }
                Err(Error::Persist(err))
            }
        }
    }
}

enum CollectOutcome {
    Exhausted,
    CollectFailed(CollectError),
    PersistFailed(PersistError),
}

async fn create_or_abort<T>(
    handles: &[GeneratorHandle],
    persister: std::result::Result<T, PersistError>,
) -> Result<T> {
    match persister {
        Ok(persister) => Ok(persister),
        Err(err) => {
            tracing::info!(
                "Failed to set up `{}`; sending Abort signal to ready load generator instances...",
                err.name
            );
            for handle in handles {
                if let Err(send_err) = handle.send_abort().await {
                    tracing::error!("Failed to send abort signal: {send_err}");
                }
            }
            Err(Error::Persist(err))
        }
    }
}

fn open_persister<P>(
    output_dir: &OutputDir,
    name: &'static str,
    new: impl FnOnce(std::fs::File) -> std::result::Result<P, PersistError>,
) -> std::result::Result<P, PersistError> {
    output_dir
        .create_file(name)
        .map_err(|source| PersistError { name, source })
        .and_then(new)
}

async fn collect_and_persist(
    handles: &mut Vec<GeneratorHandle>,
    report_tx: &tokio::sync::mpsc::Sender<IntervalReport>,
    error_rx: &mut tokio::sync::oneshot::Receiver<PersistError>,
) -> CollectOutcome {
    loop {
        tokio::select! {
            outcome = collect_reports(handles) => match outcome {
                Ok(Some(report)) => {
                    if report_tx.send(report).await.is_err() {
                        break CollectOutcome::PersistFailed(
                            error_rx.try_recv().expect(
                                "writer task panicked before signaling a persister error",
                            ),
                        );
                    }
                }
                Ok(None) => break CollectOutcome::Exhausted,
                Err(err) => break CollectOutcome::CollectFailed(err),
            },
            signaled = &mut *error_rx => {
                break CollectOutcome::PersistFailed(
                    signaled.expect("writer task panicked before signaling a persister error"),
                );
            }
        }
    }
}

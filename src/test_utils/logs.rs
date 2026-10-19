use std::io::Write;
use std::sync::{Arc, Mutex};

use tracing::subscriber::DefaultGuard;

/// Captures log output of the current thread while it is alive.
///
/// Only events emitted on the thread that created the capture are recorded,
/// so the code under test must run on that thread, e.g. by awaiting it
/// directly in a current-thread `#[tokio::test]`.
pub struct LogCapture {
    buffer: Arc<Mutex<Vec<u8>>>,
    _guard: DefaultGuard,
}

impl LogCapture {
    pub fn start() -> Self {
        let buffer = Arc::new(Mutex::new(Vec::new()));
        let writer = BufferWriter(Arc::clone(&buffer));
        let subscriber = tracing_subscriber::fmt()
            .with_writer(move || writer.clone())
            .with_ansi(false)
            .finish();
        Self {
            buffer,
            _guard: tracing::subscriber::set_default(subscriber),
        }
    }

    /// Returns everything logged so far.
    pub fn contents(&self) -> String {
        String::from_utf8_lossy(&self.buffer.lock().unwrap()).into_owned()
    }
}

#[derive(Clone)]
struct BufferWriter(Arc<Mutex<Vec<u8>>>);

impl Write for BufferWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn captures_events_while_alive() {
        let capture = LogCapture::start();
        tracing::warn!("captured");
        drop(capture);

        let after = LogCapture::start();
        assert!(!after.contents().contains("captured"));
    }

    #[test]
    fn returns_logged_messages() {
        let capture = LogCapture::start();

        tracing::warn!("something happened");

        assert!(capture.contents().contains("something happened"));
    }
}

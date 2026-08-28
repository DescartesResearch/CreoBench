use time::macros::format_description;
use tracing_subscriber::EnvFilter;

pub fn setup() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("debug"));

    tracing_subscriber::fmt()
        .with_timer(tracing_subscriber::fmt::time::UtcTime::new(
            format_description!(
                "[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:3]Z"
            ),
        ))
        .with_level(true)
        .with_target(false)
        .with_ansi(true)
        .with_env_filter(filter)
        .init();
}

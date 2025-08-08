// src/main.rs

// Import everything public from our 'tui_app' crate (which will be defined in lib.rs)
use ratatai::run;
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Setup logging
    let file_appender = tracing_appender::rolling::daily("logs", "ratatai.log");
    let (non_blocking_appender, _guard) = tracing_appender::non_blocking(file_appender);

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().with_writer(non_blocking_appender))
        .init();

    tracing::info!("Application starting");

    // Call the main function of our application defined in lib.rs
    let result = run().await;

    tracing::info!("Application ending");

    result?;
    Ok(())
}

// src/main.rs

use anyhow::bail;
// Import everything public from our 'tui_app' crate (which will be defined in lib.rs)
use ratatai::{exit_gui, run, start_gui};
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Setup logging
    let file_appender = tracing_appender::rolling::daily("logs", "ratatai.log");
    let (non_blocking_appender, _guard) = tracing_appender::non_blocking(file_appender);

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::registry()
        .with(filter)
        .with(
            fmt::layer()
                .with_writer(non_blocking_appender)
                .with_ansi(false),
        )
        .init();

    tracing::info!("Application starting");

    // Initialize Crossterm and Ratatui terminal
    let mut terminal = start_gui()?;
    // Call the main function of our application defined in lib.rs
    match run(&mut terminal).await {
        Ok(_) => {
            exit_gui(terminal)?;
        }
        Err(e) => {
            // Attempt to restore terminal to display the error
            exit_gui(terminal)?;
            bail!(e);
        }
    }

    tracing::info!("Application ending");

    Ok(())
}

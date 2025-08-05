// src/main.rs

// Import everything public from our 'tui_app' crate (which will be defined in lib.rs)
use ratatai::run;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Call the main function of our application defined in lib.rs
    run().await?;
    Ok(())
}

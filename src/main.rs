// src/main.rs

// Importe tout ce qui est public de notre crate 'tui_app' (qui sera défini dans lib.rs)
use ratatai::run;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Appelle la fonction principale de notre application définie dans lib.rs
    run().await?;
    Ok(())
}

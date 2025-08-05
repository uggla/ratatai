// src/lib.rs

use crossterm::{
    event::{self, Event as CrosstermEvent},
    execute,
    terminal::{
        Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
        enable_raw_mode,
    },
};
use google_ai_rs::Client;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use std::{
    io::{Write, stdout},
    sync::Arc,
    time::Duration,
};
use tokio::time::sleep;

// Import the modules we are going to create
mod ai;
mod app;
mod events;
mod ui;

use ai::get_gemini_response_static;
use ui::draw_ui;

use crate::{
    app::App,
    events::{QuitApp, handle_key_events},
};

/// Main function of the TUI application.
pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
    dotenv::dotenv().ok();
    // The API key is not strictly necessary at the moment, but we keep it for later.
    let api_key = std::env::var("GOOGLE_API_KEY")
        .expect("GOOGLE_API_KEY not set in .env file or environment variables");

    // Initialize Crossterm and Ratatui terminal
    enable_raw_mode()?;
    execute!(stdout(), Clear(ClearType::All))?;
    execute!(stdout(), EnterAlternateScreen)?;

    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
    terminal.hide_cursor()?;

    // Create a new instance of our application
    let mut app = App::new(Client::new(api_key.into()).await?);

    // let client_for_spawn = Arc::clone(&client);
    let gemini_response_text_for_spawn = Arc::clone(&app.gemini_response);

    // Start the asynchronous task for the "Gemini response" (static for now)
    tokio::spawn(async move {
        // let model = GenerativeModel::new(&client_for_spawn, "gemini-2.5-flash");
        // match get_gemini_response(model).await {
        match get_gemini_response_static().await {
            Ok(response) => {
                let mut response_guard = gemini_response_text_for_spawn.lock().unwrap();
                // *response_guard = response;
                // *response_guard = response.text();
                *response_guard = response;
            }
            Err(e) => {
                let mut response_guard = gemini_response_text_for_spawn.lock().unwrap();
                *response_guard = format!("Error while fetching the response: {e}");
            }
        }
    });

    // Main application loop
    loop {
        // Draw the user interface by passing the reference to the app object
        terminal.draw(|f| draw_ui(f, &mut app))?;

        // Handle input events
        if event::poll(Duration::from_millis(64))? {
            if let CrosstermEvent::Key(key) = event::read()? {
                let exit = handle_key_events(key, &mut app, &mut terminal).await?;
                if exit == QuitApp::Yes {
                    break;
                }
            }
            sleep(Duration::from_millis(32)).await;
        }
    }
    // Final cleanup before exiting
    disable_raw_mode()?;
    execute!(stdout(), LeaveAlternateScreen)?;
    stdout().flush()?;
    terminal.show_cursor()?;

    Ok(())
}

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
use launchpad_api_client::{BugTaskEntry, LaunchpadError, StatusFilter, get_project_bug_tasks};
use ratatui::{
    Terminal,
    widgets::{Row, ScrollbarState},
};
use ratatui::{backend::CrosstermBackend, widgets::Cell};
use regex::Regex;
use std::{
    io::{Empty, Write, stdout},
    sync::Arc,
    time::Duration,
};
use tokio::{
    sync::mpsc::{self, Sender},
    time::{Instant, sleep},
};
use tracing::{debug, error};

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

const PROJECT: &str = "nova";

#[derive(Debug)]
enum LpMessage {
    Bugs(Box<[BugTaskEntry]>),
    Bug(Box<launchpad_api_client::LaunchpadBug>),
    Error(LaunchpadError),
}

/// Main function of the TUI application.
pub async fn run() -> anyhow::Result<()> {
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

    let (lp_sender, mut lp_receiver) = mpsc::channel::<LpMessage>(5);

    // Create a new instance of our application
    let mut app = App::new(
        Client::new(api_key.into()).await?,
        launchpad_api_client::client::ReqwestClient::new(),
        lp_sender,
    );

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

    app.get_bugs(PROJECT.to_string());
    let project_regexp = Regex::new(r#"#(\d+).*?OpenStack Compute \(nova\):\s+"([^"]+)""#).unwrap();

    let tick_rate = Duration::from_millis(120);
    let mut last_tick = Instant::now();
    // Main application loop
    loop {
        // Draw the user interface by passing the reference to the app object
        terminal.draw(|f| draw_ui(f, &mut app))?;

        match lp_receiver.try_recv() {
            Err(tokio::sync::mpsc::error::TryRecvError::Empty) => {}
            Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {}
            Ok(msg) => match msg {
                LpMessage::Bugs(bugs) => app.update_bugs(bugs, &project_regexp),
                _ => unimplemented!(),
            },
        };
        // Handle input events
        let timeout = tick_rate.saturating_sub(last_tick.elapsed());
        if event::poll(timeout)? {
            if let CrosstermEvent::Key(key) = event::read()? {
                let exit = handle_key_events(key, &mut app, &mut terminal).await?;
                if exit == QuitApp::Yes {
                    break;
                }
            }
        }
        if last_tick.elapsed() >= tick_rate {
            last_tick = Instant::now();
        }
    }
    // Final cleanup before exiting
    disable_raw_mode()?;
    execute!(stdout(), LeaveAlternateScreen)?;
    stdout().flush()?;
    terminal.show_cursor()?;

    Ok(())
}

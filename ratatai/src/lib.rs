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
use ratatui::backend::CrosstermBackend;
use ratatui::{Terminal, widgets::ScrollbarState};
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

    let (launchpad_to_app_tx, mut launchpad_to_app_rx) = mpsc::channel::<LpMessage>(5);

    get_bugs(launchpad_to_app_tx, &mut app);

    let tick_rate = Duration::from_millis(120);
    let mut last_tick = Instant::now();
    // Main application loop
    loop {
        // Draw the user interface by passing the reference to the app object
        terminal.draw(|f| draw_ui(f, &mut app))?;

        match launchpad_to_app_rx.try_recv() {
            Err(tokio::sync::mpsc::error::TryRecvError::Empty) => {}
            Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {}
            Ok(msg) => match msg {
                LpMessage::Bugs(bugs) => {
                    app.bug_table_items = bugs;
                    app.spinner_enabled = false;
                    app.bug_table_state.select(Some(0));
                    app.bug_table_scrollbar_state = ScrollbarState::new(app.bug_table_items.len());
                }
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

fn get_bugs(launchpad_to_app_tx: Sender<LpMessage>, app: &mut App) {
    app.spinner_enabled = true;
    tokio::spawn(async move {
        debug!("Task to get bugs started");
        let client = launchpad_api_client::client::ReqwestClient::new();

        match get_project_bug_tasks(&client, "nova", Some(StatusFilter::New)).await {
            Ok(mut bug_tasks) => {
                bug_tasks.sort_by(|a, b| b.date_created.cmp(&a.date_created));

                if let Err(e) = launchpad_to_app_tx
                    .send(LpMessage::Bugs(bug_tasks.into_boxed_slice()))
                    .await
                {
                    error!("Fail to send message, error {e}");
                }
            }
            Err(e) => {
                if let Err(e) = launchpad_to_app_tx.send(LpMessage::Error(e)).await {
                    error!("Fail to send message, error {e}");
                }
            }
        }
        debug!("Task to get bugs completed");
    });
}

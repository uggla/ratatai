// src/lib.rs

use anyhow::bail;
use crossterm::{
    ExecutableCommand,
    event::{self, Event as CrosstermEvent},
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use google_ai_rs::Client;
use launchpad_api_client::{BugTaskEntry, LaunchpadError};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use regex::Regex;
use std::{
    io::{Write, stdout},
    sync::Arc,
    time::Duration,
};
use tokio::{
    sync::mpsc::{self},
    time::Instant,
};

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
pub async fn run(terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>) -> anyhow::Result<()> {
    dotenv::dotenv().ok();
    // The API key is not strictly necessary at the moment, but we keep it for later.
    let api_key = std::env::var("GOOGLE_API_KEY")?;

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
                LpMessage::Bug(bug) => app.update_bug(*bug),
                LpMessage::Error(e) => bail!(e),
            },
        };
        // Handle input events
        let timeout = tick_rate.saturating_sub(last_tick.elapsed());
        if event::poll(timeout)? {
            if let CrosstermEvent::Key(key) = event::read()? {
                let exit = handle_key_events(key, &mut app, terminal).await?;
                if exit == QuitApp::Yes {
                    break;
                }
            }
        }
        if last_tick.elapsed() >= tick_rate {
            last_tick = Instant::now();
        }
    }
    Ok(())
}

pub fn exit_gui(
    mut terminal: Terminal<CrosstermBackend<std::io::Stdout>>,
) -> Result<(), anyhow::Error> {
    disable_raw_mode()?;
    ExecutableCommand::execute(&mut stdout(), LeaveAlternateScreen)?;
    stdout().flush()?;
    terminal.show_cursor()?;
    Ok(())
}

pub fn start_gui() -> Result<Terminal<CrosstermBackend<std::io::Stdout>>, anyhow::Error> {
    ExecutableCommand::execute(&mut stdout(), EnterAlternateScreen)?;
    enable_raw_mode()?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
    terminal.hide_cursor()?;
    Ok(terminal)
}

// src/lib.rs

// Import the modules we are going to create
mod ai;
mod ai_backend;
mod app;
mod events;
mod ui;

use anyhow::bail;
use crossterm::{
    ExecutableCommand,
    event::{self, Event as CrosstermEvent},
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use launchpad_api_client::{BugTaskEntry, LaunchpadError};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use regex::Regex;
use std::{
    io::{Write, stdout},
    time::Duration,
};

use tokio::{
    sync::mpsc::{self, error},
    time::Instant,
};
use tracing::debug;
use ui::draw_ui;

use crate::{
    ai::{fetch_roster_bug_ids, fetch_supported_versions, get_system_instruction},
    ai_backend::configured_provider,
    app::{AiResponse, App},
    events::{QuitApp, handle_key_events},
};

const PROJECT: &str = "nova";

#[derive(Debug)]
enum LpMessage {
    Bugs(Box<[BugTaskEntry]>),
    Bug(u64, Box<launchpad_api_client::LaunchpadBug>),
    BugError(u64, LaunchpadError),
    BugListMessage(String),
    Error(LaunchpadError),
}

/// Main function of the TUI application.
pub async fn run(terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>) -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    let ai_provider = configured_provider().await?;

    let (lp_sender, mut lp_receiver) = mpsc::channel::<LpMessage>(5);
    let (ai_sender, mut ai_receiver) = mpsc::channel::<AiResponse>(5);

    // Fetch supported OpenStack versions and triage roster in parallel
    let (supported_versions, roster_bug_ids) =
        tokio::join!(fetch_supported_versions(), fetch_roster_bug_ids());
    let system_instruction = get_system_instruction(&supported_versions);
    debug!("System instruction: {system_instruction}");
    let mut app = App::new(
        ai_provider,
        system_instruction,
        launchpad_api_client::client::ReqwestClient::new(),
        lp_sender,
        ai_sender,
    );
    app.roster_bug_ids = roster_bug_ids;

    app.get_bugs(PROJECT.to_string());
    let project_regexp = Regex::new(r#"#(\d+).*?OpenStack Compute \(nova\):\s+"([^"]+)""#).unwrap();

    let tick_rate = Duration::from_millis(120);
    let mut last_tick = Instant::now();
    // Main application loop
    loop {
        // Draw the user interface by passing the reference to the app object
        terminal.draw(|f| draw_ui(f, &mut app))?;

        // Manage message from launchpad
        match lp_receiver.try_recv() {
            Err(error::TryRecvError::Empty) => {}
            Err(error::TryRecvError::Disconnected) => {}
            Ok(msg) => match msg {
                LpMessage::Bugs(bugs) => app.update_bugs(bugs, &project_regexp),
                LpMessage::Bug(generation, bug) => app.update_bug(generation, *bug),
                LpMessage::BugError(generation, e) => {
                    if app.is_current_bug_request(generation) {
                        bail!(e);
                    }
                }
                LpMessage::BugListMessage(message) => app.update_bug_list_message(message),
                LpMessage::Error(e) => bail!(e),
            },
        };

        // Manage responses from the active bug's AI session.
        match ai_receiver.try_recv() {
            Err(error::TryRecvError::Empty) => {}
            Err(error::TryRecvError::Disconnected) => {}
            Ok(response) => app.apply_ai_response(response),
        };

        // Handle input events
        let timeout = tick_rate.saturating_sub(last_tick.elapsed());
        if event::poll(timeout)?
            && let CrosstermEvent::Key(key) = event::read()?
        {
            let exit = handle_key_events(key, &mut app, terminal).await?;
            if exit == QuitApp::Yes {
                break;
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

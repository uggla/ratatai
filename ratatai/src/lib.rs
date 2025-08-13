// src/lib.rs

// Import the modules we are going to create
mod ai;
mod app;
mod events;
mod join_monitor;
mod ui;

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
    time::Duration,
};
use tokio::{
    sync::mpsc::{self, error},
    time::Instant,
};
use tracing::{debug, error, info};
use ui::draw_ui;

use crate::{
    app::App,
    events::{QuitApp, handle_key_events},
    join_monitor::{JoinHandleMonitor, check_monitor},
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
    dotenvy::dotenv().ok();
    let api_key = std::env::var("GOOGLE_API_KEY")?;

    let (lp_sender, mut lp_receiver) = mpsc::channel::<LpMessage>(5);
    let (app_sender, mut app_receiver) = mpsc::channel::<String>(5);
    let (chat_sender, chat_receiver) = mpsc::channel::<String>(5);

    // Create a new instance of our application
    let mut app = App::new(
        Client::new(api_key).await?,
        launchpad_api_client::client::ReqwestClient::new(),
        lp_sender,
        app_sender,
        chat_receiver,
    );

    // Start the asynchronous task for gemini chat"
    let client = app.gemini_client.clone();

    let chat_task = tokio::spawn(async move {
        let chat = client.generative_model("gemini-2.5-flash");
        let mut session = chat.start_chat();
        info!("Chat started");

        while let Some(msg) = app_receiver.recv().await {
            info!("Chat message received");
            debug!("Message: {msg}");

            match session.send_message(msg).await {
                Ok(response) => {
                    if let Err(e) = chat_sender.send(response.text()).await {
                        error!("Error sending message: {e}");
                        break;
                    }
                }
                Err(e) => {
                    error!("Error calling gemini: {e}");
                    break;
                }
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        }

        info!("Chat terminated");
    });

    let mut monitor = JoinHandleMonitor::new(chat_task);

    app.get_bugs(PROJECT.to_string());
    let project_regexp = Regex::new(r#"#(\d+).*?OpenStack Compute \(nova\):\s+"([^"]+)""#).unwrap();

    let tick_rate = Duration::from_millis(120);
    let mut last_tick = Instant::now();
    // Main application loop
    loop {
        if check_monitor(&mut monitor) {
            break;
        }
        // Draw the user interface by passing the reference to the app object
        terminal.draw(|f| draw_ui(f, &mut app))?;

        // Manage message from launchpad
        match lp_receiver.try_recv() {
            Err(error::TryRecvError::Empty) => {}
            Err(error::TryRecvError::Disconnected) => {}
            Ok(msg) => match msg {
                LpMessage::Bugs(bugs) => app.update_bugs(bugs, &project_regexp),
                LpMessage::Bug(bug) => app.update_bug(*bug),
                LpMessage::Error(e) => bail!(e),
            },
        };

        // Manage message from gemini chat
        match app.chat_receiver.try_recv() {
            Err(error::TryRecvError::Empty) => {}
            Err(error::TryRecvError::Disconnected) => {}
            Ok(msg) => {
                info!("Chat response received");
                debug!("Response: {msg}");
                app.update_bug_reply(msg);
            }
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

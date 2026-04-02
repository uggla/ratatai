// src/lib.rs

// Import the modules we are going to create
mod ai;
mod app;
mod events;
mod ui;

use anyhow::bail;
use crossterm::{
    ExecutableCommand,
    event::{self, Event as CrosstermEvent},
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use google_ai_rs::{Client, Error as GeminiError};
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
use tracing::{debug, error, info, warn};
use ui::draw_ui;

use crate::{
    ai::{fetch_roster_bug_ids, fetch_supported_versions, get_system_instruction},
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
    dotenvy::dotenv().ok();
    let api_key = std::env::var("GEMINI_API_KEY")?;

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

    // Fetch supported OpenStack versions and triage roster in parallel
    let (supported_versions, roster_bug_ids) =
        tokio::join!(fetch_supported_versions(), fetch_roster_bug_ids());
    let system_instruction = get_system_instruction(&supported_versions);
    debug!("System instruction: {system_instruction}");
    app.roster_bug_ids = roster_bug_ids;

    // Start the asynchronous task for gemini chat
    let client = app.gemini_client.clone();

    let chat_task = tokio::spawn(async move {
        let chat = client
            .generative_model("gemini-3-flash-preview")
            .with_system_instruction(system_instruction);
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
                    let user_msg = extract_error_message(&e);
                    if let Err(send_err) = chat_sender.send(user_msg).await {
                        error!("Error sending error message: {send_err}");
                        break;
                    }
                }
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        }

        info!("Chat terminated");
    });

    app.get_bugs(PROJECT.to_string());
    let project_regexp = Regex::new(r#"#(\d+).*?OpenStack Compute \(nova\):\s+"([^"]+)""#).unwrap();

    let tick_rate = Duration::from_millis(120);
    let mut last_tick = Instant::now();
    // Main application loop
    loop {
        if chat_task.is_finished() {
            return chat_task_result_to_err(chat_task.await);
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

fn chat_task_result_to_err(res: Result<(), tokio::task::JoinError>) -> anyhow::Result<()> {
    match res {
        Ok(_) => {
            warn!("⚠️ Chat task stopped.");
            Err(anyhow::anyhow!(
                "😵 Chat task stopped unexpectedly. See logs for details."
            ))
        }
        Err(e) => {
            error!("💥 Chat task panicked : {e}");
            Err(anyhow::anyhow!(
                "💥 Chat task panicked: {e}. See logs for details."
            ))
        }
    }
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

fn extract_error_message(err: &GeminiError) -> String {
    format!("⚠️ {}", extract_user_message(&err.to_string()))
}

/// Extract the human-readable message from a tonic::Status Display format.
/// Input format: `... message: "human readable message", details: ...`
/// Returns the extracted message, or the full string if no message field is found.
fn extract_user_message(full: &str) -> &str {
    if let Some(start) = full.find("message: \"") {
        let msg_start = start + "message: \"".len();
        if let Some(end) = full[msg_start..].find('"') {
            return &full[msg_start..msg_start + end];
        }
    }
    full
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_user_message_from_tonic_status() {
        let input = r#"Service Error: API Error: Status: status: Unavailable, message: "This model is currently experiencing high demand. Spikes in demand are usually temporary. Please try again later.", details: [], metadata: MetadataMap { headers: {"content-type": "application/grpc"} }"#;
        assert_eq!(
            extract_user_message(input),
            "This model is currently experiencing high demand. Spikes in demand are usually temporary. Please try again later."
        );
    }

    #[test]
    fn test_extract_user_message_no_message_field() {
        let input = "Some other error format without message field";
        assert_eq!(extract_user_message(input), input);
    }

    #[test]
    fn test_extract_user_message_empty_message() {
        let input = r#"Status: status: Unknown, message: "", details: []"#;
        assert_eq!(extract_user_message(input), "");
    }
}

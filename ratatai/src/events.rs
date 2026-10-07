use anyhow::bail;
use crossterm::{
    ExecutableCommand,
    event::{KeyCode, KeyEvent, KeyEventKind},
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use std::env;
use tempfile::NamedTempFile;
use tokio::{fs::File, io::AsyncReadExt, process::Command};
use tracing::error;

use crate::{
    PROJECT,
    app::{ActivePanel, AiTarget, App, Screen},
};

#[derive(Debug, PartialEq)]
pub(crate) enum QuitApp {
    Yes,
    No,
}

// Extracted function for handling key events
pub async fn handle_key_events(
    key: KeyEvent,
    app: &mut App,
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
) -> anyhow::Result<QuitApp> {
    if key.kind == KeyEventKind::Press {
        if let QuitApp::Yes = handle_global_keys(key, app)? {
            return Ok(QuitApp::Yes);
        }

        if let QuitApp::Yes = match app.current_screen {
            Screen::BugList => handle_bug_list_screen_keys(key, app)?,
            Screen::BugEditing => handle_bug_editing_screen_keys(key, app)?,
        } {
            return Ok(QuitApp::Yes);
        }

        if let QuitApp::Yes = match app.current_screen {
            Screen::BugList => match app.active_panel {
                ActivePanel::Left => handle_bug_table(key, app).await?,
                ActivePanel::Right => handle_bug_description(key, app, terminal).await?,
            },
            Screen::BugEditing => match app.active_panel {
                ActivePanel::Left => handle_bug_description(key, app, terminal).await?,
                ActivePanel::Right => handle_bug_reply(key, app, terminal).await?,
            },
        } {
            return Ok(QuitApp::Yes);
        }
    }

    Ok(QuitApp::No) // Return false if no exit condition was met
}

fn handle_global_keys(key: KeyEvent, app: &mut App) -> anyhow::Result<QuitApp> {
    match key.code {
        KeyCode::Char('s') => {
            app.toggle_spinner();
        }
        KeyCode::Char('q') => return Ok(QuitApp::Yes),
        _ => {}
    }
    Ok(QuitApp::No)
}

fn handle_bug_list_screen_keys(key: KeyEvent, app: &mut App) -> anyhow::Result<QuitApp> {
    if let KeyCode::Tab = key.code {
        if app.active_panel == ActivePanel::Right {
            app.active_panel = ActivePanel::Left
        } else {
            app.active_panel = ActivePanel::Right
        }
    }
    Ok(QuitApp::No)
}

fn handle_bug_editing_screen_keys(key: KeyEvent, app: &mut App) -> anyhow::Result<QuitApp> {
    match key.code {
        KeyCode::Esc => {
            app.end_ai_session();
            app.current_screen = Screen::BugList;
            app.active_panel = ActivePanel::Left;
        }
        KeyCode::Tab => {
            if app.active_panel == ActivePanel::Right {
                app.active_panel = ActivePanel::Left
            } else if app.ai_generation_triggered {
                app.active_panel = ActivePanel::Right
            }
        }
        _ => (),
    }
    Ok(QuitApp::No)
}

// Bug table is activated
async fn handle_bug_table(key: KeyEvent, app: &mut App) -> anyhow::Result<QuitApp> {
    match key.code {
        KeyCode::Up => app.bug_table_previous_item(),
        KeyCode::Down => app.bug_table_next_item(),
        KeyCode::PageUp => app.bug_table_page_up_item(),
        KeyCode::PageDown => app.bug_table_page_down_item(),
        KeyCode::Home => app.bug_table_go_to_start(),
        KeyCode::End => app.bug_table_go_to_end(),
        KeyCode::Char('r') => app.get_bugs(PROJECT.to_string()),
        KeyCode::Enter => {
            if let Some(index) = app.bug_table_state.selected()
                && let Some(bug_entry) = app.bug_table_items.get(index)
            {
                app.get_bug(bug_entry.get_id());
            }
        }
        _ => {}
    }
    Ok(QuitApp::No)
}

async fn handle_bug_description(
    key: KeyEvent,
    app: &mut App,
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
) -> anyhow::Result<QuitApp> {
    match key.code {
        KeyCode::Up => {
            app.bug_desc_scroll = app.bug_desc_scroll.saturating_sub(1);
            app.bug_desc_scroll_to_end = false;
        }
        KeyCode::Down => {
            app.bug_desc_scroll = app.bug_desc_scroll.saturating_add(1);
            app.bug_desc_scroll_to_end = false;
        }
        KeyCode::PageUp => {
            app.bug_desc_scroll = app.bug_desc_scroll.saturating_sub(10);
            app.bug_desc_scroll_to_end = false;
        }
        KeyCode::PageDown => {
            app.bug_desc_scroll = app.bug_desc_scroll.saturating_add(10);
            app.bug_desc_scroll_to_end = false;
        }
        KeyCode::Home => {
            app.bug_desc_scroll = 0;
            app.bug_desc_scroll_to_end = false;
        }
        KeyCode::End => {
            app.bug_desc_scroll_to_end = true;
        }
        KeyCode::Char('v') => {
            if let Some(index) = app.bug_table_state.selected()
                && let Some(bug_entry) = app.bug_table_items.get(index)
            {
                let status = Command::new("xdg-open")
                    .arg(&bug_entry.web_link)
                    .status()
                    .await?;

                if !status.success() {
                    error!("Fail to open url: {:?}", status.code());
                }
            }
        }
        KeyCode::Char('a') => {
            if app.current_bug.is_some() {
                app.request_ai(AiTarget::BugDescription, app.bug_description_text.clone());
            }
        }
        KeyCode::Char('e') => {
            let initial_content = app.bug_description_text.clone();
            let updated = edit_content_in_editor(terminal, initial_content).await?;
            app.bug_description_text = updated;
        }
        KeyCode::Enter => {
            if app.current_screen == Screen::BugList {
                if app.current_bug.is_none() {
                    return Ok(QuitApp::No);
                }
                app.start_ai_session();
                app.current_screen = Screen::BugEditing;
                app.active_panel = ActivePanel::Left;
                app.bug_reply_text = "Press Enter to generate AI response.".to_string();
                app.ai_generation_triggered = false;
            } else if app.current_screen == Screen::BugEditing {
                let bug_content = app.bug_description_text.clone();
                app.request_ai(AiTarget::Draft, bug_content);
                app.bug_reply_text = "Waiting for AI response...".to_string();
                app.ai_generation_triggered = true;
            }
        }
        _ => {}
    }
    Ok(QuitApp::No)
}

async fn handle_bug_reply(
    key: KeyEvent,
    app: &mut App,
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
) -> anyhow::Result<QuitApp> {
    match key.code {
        // KeyCode::Up => {
        //     app.bug_desc_scroll = app.bug_desc_scroll.saturating_sub(1);
        //     app.bug_desc_scroll_to_end = false;
        // }
        // KeyCode::Down => {
        //     app.bug_desc_scroll = app.bug_desc_scroll.saturating_add(1);
        //     app.bug_desc_scroll_to_end = false;
        // }
        // KeyCode::PageUp => {
        //     app.bug_desc_scroll = app.bug_desc_scroll.saturating_sub(10);
        //     app.bug_desc_scroll_to_end = false;
        // }
        // KeyCode::PageDown => {
        //     app.bug_desc_scroll = app.bug_desc_scroll.saturating_add(10);
        //     app.bug_desc_scroll_to_end = false;
        // }
        // KeyCode::Home => {
        //     app.bug_desc_scroll = 0;
        //     app.bug_desc_scroll_to_end = false;
        // }
        // KeyCode::End => {
        //     app.bug_desc_scroll_to_end = true;
        // }
        KeyCode::Enter => {
            app.request_ai(AiTarget::Draft, app.bug_reply_text.clone());
        }
        KeyCode::Char('e') => {
            let initial_content = app.bug_reply_text.clone();
            let updated = edit_content_in_editor(terminal, initial_content).await?;
            app.bug_reply_text = updated;
        }
        _ => {}
    }
    Ok(QuitApp::No)
}

async fn edit_content_in_editor<S>(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    content: S,
) -> anyhow::Result<String>
where
    S: Into<String>,
{
    // Prepare the file with the given content
    let (file_path, _file) = tokio::task::spawn_blocking({
        let content = content.into();
        move || {
            let mut temp = NamedTempFile::new()?;
            std::io::Write::write_all(&mut temp, content.as_bytes())?;
            let path = temp.path().to_owned();
            Ok::<_, std::io::Error>((path, temp))
        }
    })
    .await??;

    // Exit Ratatui mode
    ExecutableCommand::execute(&mut std::io::stdout(), LeaveAlternateScreen)?;
    disable_raw_mode()?;

    // Launch the external editor
    let editor = env::var("EDITOR").unwrap_or_else(|_| "nvim".to_string());
    let status = Command::new(&editor).arg(&file_path).status().await?;
    if !status.success() {
        bail!("The editor exited with an error: {:?}", status.code());
    }

    // Read updated content
    let mut updated_content = String::new();
    let mut file = File::open(file_path).await?;
    AsyncReadExt::read_to_string(&mut file, &mut updated_content).await?;

    // Re-enable Ratatui mode
    std::io::stdout().execute(EnterAlternateScreen)?;
    enable_raw_mode()?;
    terminal.clear()?;
    terminal.hide_cursor()?;

    Ok(updated_content)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use async_trait::async_trait;
    use crossterm::event::KeyModifiers;
    use tokio::sync::mpsc;

    use super::*;
    use crate::ai_backend::{AiProvider, AiSession};

    struct FakeProvider;

    impl AiProvider for FakeProvider {
        fn display_name(&self) -> &'static str {
            "Test AI"
        }

        fn start_session(&self, _instruction: String) -> Box<dyn AiSession> {
            Box::new(FakeSession)
        }
    }

    struct FakeSession;

    #[async_trait]
    impl AiSession for FakeSession {
        async fn send(&mut self, prompt: String) -> anyhow::Result<String> {
            Ok(prompt)
        }
    }

    #[tokio::test]
    async fn tab_keeps_session_and_escape_from_triage_ends_it() {
        let (lp_sender, _) = mpsc::channel(1);
        let (ai_sender, mut ai_receiver) = mpsc::channel(1);
        let mut app = App::new(
            Arc::new(FakeProvider),
            "rules".into(),
            launchpad_api_client::client::ReqwestClient::new(),
            lp_sender,
            ai_sender,
        );
        app.start_ai_session();
        app.active_panel = ActivePanel::Right;

        handle_bug_list_screen_keys(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE), &mut app)
            .unwrap();
        assert_eq!(app.active_panel, ActivePanel::Left);
        app.request_ai(AiTarget::Draft, "still active".into());
        assert_eq!(
            ai_receiver.recv().await.unwrap().result.unwrap(),
            "still active"
        );

        app.current_screen = Screen::BugEditing;
        app.bug_description_text = "bug details".into();
        handle_bug_editing_screen_keys(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), &mut app)
            .unwrap();
        assert_eq!(app.current_screen, Screen::BugList);
        assert_eq!(app.bug_description_text, "bug details");
        app.request_ai(AiTarget::Draft, "should not send".into());
        assert!(ai_receiver.try_recv().is_err());
    }
}

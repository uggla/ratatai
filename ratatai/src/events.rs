use anyhow::bail;
use crossterm::{
    ExecutableCommand,
    event::{KeyCode, KeyEvent, KeyEventKind},
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use google_ai_rs::GenerativeModel;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use std::{env, sync::Arc};
use tempfile::NamedTempFile;
use tokio::{fs::File, io::AsyncReadExt, process::Command};
use tracing::error;

use crate::{
    PROJECT,
    ai::{get_gemini_response, get_initial_prompt},
    app::{ActivePanel, App, Screen},
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
            app.current_screen = Screen::BugList;
            app.active_panel = ActivePanel::Left;
        }
        KeyCode::Tab => {
            if app.active_panel == ActivePanel::Right {
                app.active_panel = ActivePanel::Left
            } else {
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
            if let Some(index) = app.bug_table_state.selected() {
                if let Some(bug_entry) = app.bug_table_items.get(index) {
                    app.get_bug(bug_entry.get_id());
                }
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
            if let Some(index) = app.bug_table_state.selected() {
                if let Some(bug_entry) = app.bug_table_items.get(index) {
                    let status = Command::new("xdg-open")
                        .arg(&bug_entry.web_link)
                        .status()
                        .await?;

                    if !status.success() {
                        error!("Fail to open url: {:?}", status.code());
                    }
                }
            }
        }
        KeyCode::Char('a') => {
            let client = Arc::clone(&app.gemini_client);
            let gemini_response_text_for_spawn = Arc::clone(&app.gemini_response);
            let prompt = { gemini_response_text_for_spawn.lock().unwrap().clone() };

            tokio::spawn(async move {
                let model = GenerativeModel::new(&client, "gemini-2.5-flash");

                match get_gemini_response(model, prompt).await {
                    Ok(response) => {
                        let mut response_guard = gemini_response_text_for_spawn.lock().unwrap();
                        *response_guard = response.text();
                    }
                    Err(e) => {
                        let mut response_guard = gemini_response_text_for_spawn.lock().unwrap();
                        *response_guard = format!("Error while fetching the response: {e}");
                    }
                }
            });
            // Ai request
        }
        KeyCode::Char('e') => {
            let initial_content = { app.gemini_response.lock().unwrap().clone() };
            let updated = edit_content_in_editor(terminal, initial_content).await?;
            {
                let mut response_guard = app.gemini_response.lock().unwrap();
                *response_guard = updated;
            }
        }
        KeyCode::Enter => {
            if app.current_screen == Screen::BugList {
                app.current_screen = Screen::BugEditing;
                app.active_panel = ActivePanel::Left;
                app.bug_reply_text = "No bug replied yet.".to_string();
            } else {
                let bug_guard = { app.gemini_response.lock().unwrap().clone() };

                let prompt = format!("{}\n{}", get_initial_prompt(), bug_guard);
                app.app_sender.send(prompt).await?;
                app.spinner_enabled = true;
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
            app.app_sender.send(app.bug_reply_text.clone()).await?;
            app.spinner_enabled = true;
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

use anyhow::bail;
use crossterm::{
    ExecutableCommand,
    event::{KeyCode, KeyEvent, KeyEventKind},
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use google_ai_rs::GenerativeModel;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use std::{env, io::stdout, sync::Arc};
use tempfile::NamedTempFile;
use tokio::{fs::File, io::AsyncReadExt, process::Command};
use tracing::error;

use crate::{
    PROJECT,
    ai::get_gemini_response,
    app::{ActivePanel, App, Screen},
    ui::draw_ui,
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
                ActivePanel::Left => handle_bug_table(key, app)?,
                ActivePanel::Right => handle_bug_description(key, app, terminal).await?,
            },
            Screen::BugEditing => match app.active_panel {
                ActivePanel::Left => handle_bug_description(key, app, terminal).await?,
                ActivePanel::Right => QuitApp::No,
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
            app.active_panel = ActivePanel::Right;
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
fn handle_bug_table(key: KeyEvent, app: &mut App) -> anyhow::Result<QuitApp> {
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
                if let Some(bug) = app.bug_table_items.get(index) {
                    let mut gemini_response = app.gemini_response.lock().unwrap();
                    *gemini_response = bug.bug_link.clone();
                    app.bug_desc_scroll = 0;
                    app.bug_desc_scroll_to_end = false;
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
        KeyCode::Char('r') => {
            app.current_screen = Screen::BugEditing;
            app.active_panel = ActivePanel::Left;
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
            // Edit the content
            let content_to_edit = app.gemini_response.lock().unwrap().clone();

            // let mut temp_file = NamedTempFile::new()?;
            // std::io::Write::write_all(&mut temp_file, content_to_edit.as_bytes())?;
            // let file_path = temp_file.path().to_path_buf();

            let (file_path, _file) = tokio::task::spawn_blocking({
                let content = content_to_edit.to_owned();
                move || {
                    let mut temp = NamedTempFile::new()?;
                    std::io::Write::write_all(&mut temp, content.as_bytes())?;
                    let path = temp.path().to_owned();
                    Ok::<_, std::io::Error>((path, temp))
                }
            })
            .await??;

            // 1. Exit Ratatui mode
            ExecutableCommand::execute(&mut stdout(), LeaveAlternateScreen)?;
            disable_raw_mode()?;

            // 2. Launch the external editor
            let editor = env::var("EDITOR").unwrap_or_else(|_| "nvim".to_string());
            let status = Command::new(&editor).arg(&file_path).status().await?;

            if !status.success() {
                error!("The editor exited with an error: {:?}", status.code());
                bail!("The editor exited with an error: {:?}", status.code());
            }

            // Read the updated content from the temporary file
            let mut updated_content = String::new();
            let mut file = File::open(file_path).await?;
            AsyncReadExt::read_to_string(&mut file, &mut updated_content).await?;
            {
                // Update the application state with the new content
                let mut response_guard = app.gemini_response.lock().unwrap();
                *response_guard = updated_content;
            }

            // 3. Re-enable Ratatui mode
            stdout().execute(EnterAlternateScreen)?;
            enable_raw_mode()?;
            terminal.clear()?;

            terminal.draw(|f| draw_ui(f, app))?; // Redraw with the new content
            std::io::Write::flush(&mut terminal.backend_mut())?;
            terminal.hide_cursor()?;
        }
        _ => {}
    }
    Ok(QuitApp::No)
}

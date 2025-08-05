use crossterm::{
    event::{KeyCode, KeyEvent, KeyEventKind},
    execute,
    terminal::{
        Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
        enable_raw_mode,
    },
};
use google_ai_rs::GenerativeModel;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use std::{
    env,
    io::{Read, Write, stdout},
    process::Command,
    sync::Arc,
};
use tempfile::NamedTempFile;

use crate::{
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
) -> Result<QuitApp, Box<dyn std::error::Error>> {
    if key.kind == KeyEventKind::Press {
        if key.code == KeyCode::Char('s') {
            app.toggle_spinner();
        } else {
            match app.current_screen {
                Screen::BugList => {}
                Screen::BugEditing => match key.code {
                    KeyCode::Esc => {
                        app.current_screen = Screen::BugList;
                        app.active_panel = ActivePanel::Right;
                    }
                    KeyCode::Char('z') => {}
                    _ => {}
                },
            }
            match app.active_panel {
                ActivePanel::Left => match key.code {
                    KeyCode::Char('q') => return Ok(QuitApp::Yes), // Exit loop on 'q'
                    KeyCode::Up => app.previous_item(),
                    KeyCode::Down => app.next_item(),
                    KeyCode::PageUp => app.page_up_item(),
                    KeyCode::PageDown => app.page_down_item(),
                    KeyCode::Home => app.go_to_start(),
                    KeyCode::End => app.go_to_end(),
                    KeyCode::Tab => {
                        app.active_panel = ActivePanel::Right;
                        app.right_panel_scroll = 0;
                    }
                    KeyCode::Enter => {
                        app.selected_bug_index = app.table_state.selected();
                        if let Some(index) = app.selected_bug_index {
                            if let Some(bug) = app.table_items.get(index) {
                                let mut gemini_response = app.gemini_response.lock().unwrap();
                                *gemini_response = bug.description.clone();
                                app.right_panel_scroll = 0;
                                app.scroll_to_end = false;
                            }
                        }
                    }
                    _ => {}
                },
                ActivePanel::Right => match key.code {
                    KeyCode::Char('q') => return Ok(QuitApp::Yes), // Exit loop on 'q'
                    KeyCode::Up => {
                        app.right_panel_scroll = app.right_panel_scroll.saturating_sub(1);
                        app.scroll_to_end = false;
                    }
                    KeyCode::Down => {
                        app.right_panel_scroll = app.right_panel_scroll.saturating_add(1);
                        app.scroll_to_end = false;
                    }
                    KeyCode::PageUp => {
                        app.right_panel_scroll = app.right_panel_scroll.saturating_sub(10);
                        app.scroll_to_end = false;
                    }
                    KeyCode::PageDown => {
                        app.right_panel_scroll = app.right_panel_scroll.saturating_add(10);
                        app.scroll_to_end = false;
                    }
                    KeyCode::Home => {
                        app.right_panel_scroll = 0;
                        app.scroll_to_end = false;
                    }
                    KeyCode::End => {
                        app.scroll_to_end = true;
                    }
                    KeyCode::Char('r') => {
                        app.current_screen = Screen::BugEditing;
                        app.active_panel = ActivePanel::Left;
                    }
                    KeyCode::Tab => app.active_panel = ActivePanel::Left,
                    KeyCode::Char('a') => {
                        let client = Arc::clone(&app.gemini_client);
                        let gemini_response_text_for_spawn = Arc::clone(&app.gemini_response);
                        let prompt = { gemini_response_text_for_spawn.lock().unwrap().clone() };

                        tokio::spawn(async move {
                            let model = GenerativeModel::new(&client, "gemini-2.5-flash");

                            match get_gemini_response(model, prompt).await {
                                Ok(response) => {
                                    let mut response_guard =
                                        gemini_response_text_for_spawn.lock().unwrap();
                                    *response_guard = response.text();
                                }
                                Err(e) => {
                                    let mut response_guard =
                                        gemini_response_text_for_spawn.lock().unwrap();
                                    *response_guard =
                                        format!("Error while fetching the response: {e}");
                                }
                            }
                        });
                        // Ai request
                    }
                    KeyCode::Char('e') => {
                        // Edit the content
                        let content_to_edit = app.gemini_response.lock().unwrap().clone();

                        let mut temp_file = NamedTempFile::new()?;
                        temp_file.write_all(content_to_edit.as_bytes())?;
                        let file_path = temp_file.path().to_path_buf();

                        // 1. Exit Ratatui mode
                        terminal.show_cursor()?;
                        execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
                        disable_raw_mode()?;

                        // 2. Launch the external editor
                        let editor = env::var("EDITOR").unwrap_or_else(|_| "nvim".to_string());
                        let status = Command::new(&editor).arg(&file_path).status()?;

                        if !status.success() {
                            eprintln!("The editor exited with an error: {:?}", status.code());
                        }

                        // 3. Re-enable Ratatui mode
                        enable_raw_mode()?;
                        execute!(stdout(), Clear(ClearType::All), EnterAlternateScreen)?;

                        // Read the updated content from the temporary file
                        let mut updated_content = String::new();
                        std::fs::File::open(&file_path)?.read_to_string(&mut updated_content)?;
                        {
                            // Update the application state with the new content
                            let mut response_guard = app.gemini_response.lock().unwrap();
                            *response_guard = updated_content;
                        }

                        // Force a full cleanup and redraw of the TUI
                        terminal.clear()?;
                        terminal.draw(|f| draw_ui(f, app))?; // Redraw with the new content
                        terminal.backend_mut().flush()?;
                        terminal.hide_cursor()?;
                    }
                    _ => {}
                },
            }
        }
    }

    Ok(QuitApp::No) // Return false if no exit condition was met
}

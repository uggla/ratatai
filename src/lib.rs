// src/lib.rs

use crossterm::{
    event::{self, Event as CrosstermEvent, KeyCode, KeyEventKind},
    execute,
    terminal::{
        Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
        enable_raw_mode,
    },
};
use google_ai_rs::{Client, GenerativeModel};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use std::{
    env,
    io::{Read, Write, stdout},
    process::Command,
    sync::Arc,
    time::Duration,
};
use tempfile::NamedTempFile;
use tokio::time::sleep;

// Import the modules we are going to create
mod ai;
mod app;
mod ui;

use ai::get_gemini_response_static;
use ui::draw_ui;

use crate::{
    ai::get_gemini_response,
    app::{ActivePanel, App, Screen},
};

/// Main function of the TUI application.
pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
    dotenv::dotenv().ok();
    // The API key is not strictly necessary at the moment, but we keep it for later.
    let api_key = std::env::var("GOOGLE_API_KEY")
        .expect("GOOGLE_API_KEY not set in .env file or environment variables");

    let client = Arc::new(Client::new(api_key.into()).await?);

    // Initialize Crossterm and Ratatui terminal
    enable_raw_mode()?;
    execute!(stdout(), Clear(ClearType::All))?;
    execute!(stdout(), EnterAlternateScreen)?;

    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
    terminal.hide_cursor()?;

    // Create a new instance of our application
    let mut app = App::new();
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

    // Main application loop
    loop {
        // Draw the user interface by passing the reference to the app object
        terminal.draw(|f| draw_ui(f, &mut app))?;

        // Handle input events
        if event::poll(Duration::from_millis(64))? {
            if let CrosstermEvent::Key(key) = event::read()? {
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
                                // TODO: Remove this as this is just for satisfy the linter
                                KeyCode::Char('z') => {}
                                _ => {}
                            },
                        }
                        match app.active_panel {
                            ActivePanel::Left => match key.code {
                                KeyCode::Char('q') => break,
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
                                            let mut gemini_response =
                                                app.gemini_response.lock().unwrap();
                                            *gemini_response = bug.description.clone();
                                            app.right_panel_scroll = 0;
                                            app.scroll_to_end = false;
                                        }
                                    }
                                }
                                _ => {}
                            },
                            ActivePanel::Right => match key.code {
                                KeyCode::Char('q') => break,
                                KeyCode::Up => {
                                    app.right_panel_scroll =
                                        app.right_panel_scroll.saturating_sub(1);
                                    app.scroll_to_end = false;
                                }
                                KeyCode::Down => {
                                    app.right_panel_scroll =
                                        app.right_panel_scroll.saturating_add(1);
                                    app.scroll_to_end = false;
                                }
                                KeyCode::PageUp => {
                                    app.right_panel_scroll =
                                        app.right_panel_scroll.saturating_sub(10);
                                    app.scroll_to_end = false;
                                }
                                KeyCode::PageDown => {
                                    app.right_panel_scroll =
                                        app.right_panel_scroll.saturating_add(10);
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
                                    let client_for_spawn = Arc::clone(&client);
                                    let gemini_response_text_for_spawn =
                                        Arc::clone(&app.gemini_response);
                                    let prompt = {
                                        let truc = gemini_response_text_for_spawn.lock().unwrap();
                                        truc.clone()
                                    };

                                    tokio::spawn(async move {
                                        let model = GenerativeModel::new(
                                            &client_for_spawn,
                                            "gemini-2.5-flash",
                                        );

                                        match get_gemini_response(model, prompt).await {
                                            Ok(response) => {
                                                let mut response_guard =
                                                    gemini_response_text_for_spawn.lock().unwrap();
                                                *response_guard = response.text();
                                            }
                                            Err(e) => {
                                                let mut response_guard =
                                                    gemini_response_text_for_spawn.lock().unwrap();
                                                *response_guard = format!(
                                                    "Error while fetching the response: {e}"
                                                );
                                            }
                                        }
                                    });
                                    // Ai request
                                }
                                KeyCode::Char('e') => {
                                    // Edit the content
                                    let content_to_edit =
                                        app.gemini_response.lock().unwrap().clone();

                                    let mut temp_file = NamedTempFile::new()?;
                                    temp_file.write_all(content_to_edit.as_bytes())?;
                                    let file_path = temp_file.path().to_path_buf();

                                    // 1. Exit Ratatui mode
                                    terminal.show_cursor()?;
                                    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
                                    disable_raw_mode()?;

                                    // 2. Launch the external editor
                                    let editor =
                                        env::var("EDITOR").unwrap_or_else(|_| "nvim".to_string());
                                    let status = Command::new(&editor).arg(&file_path).status()?;

                                    if !status.success() {
                                        eprintln!(
                                            "The editor exited with an error: {:?}",
                                            status.code()
                                        );
                                    }

                                    // 3. Re-enable Ratatui mode
                                    enable_raw_mode()?;
                                    execute!(
                                        stdout(),
                                        Clear(ClearType::All),
                                        EnterAlternateScreen
                                    )?;

                                    // Read the updated content from the temporary file
                                    let mut updated_content = String::new();
                                    std::fs::File::open(&file_path)?
                                        .read_to_string(&mut updated_content)?;
                                    {
                                        // Update the application state with the new content
                                        let mut response_guard =
                                            app.gemini_response.lock().unwrap();
                                        *response_guard = updated_content;
                                    }

                                    // Force a full cleanup and redraw of the TUI
                                    terminal.clear()?;
                                    terminal.draw(|f| draw_ui(f, &mut app))?; // Redraw with the new content
                                    terminal.backend_mut().flush()?;
                                    terminal.hide_cursor()?;
                                }
                                _ => {}
                            },
                        }
                    }
                }
            }
            sleep(Duration::from_millis(32)).await;
        }
    }
    // Final cleanup before exiting
    disable_raw_mode()?;
    execute!(stdout(), LeaveAlternateScreen)?;
    stdout().flush()?;
    terminal.show_cursor()?;

    Ok(())
}

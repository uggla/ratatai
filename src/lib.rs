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
use ratatui::widgets::{ScrollbarState, TableState};
use std::{
    env,
    io::{Read, Write, stdout},
    process::Command,
    sync::{Arc, Mutex},
    time::Duration,
};
use tempfile::NamedTempFile;
use tokio::time::sleep;

// Import the modules we are going to create
mod ai;
mod ui;

use ai::get_gemini_response_static;
use ui::draw_ui;

use crate::ai::get_gemini_response;
use throbber_widgets_tui::ThrobberState;
use ui::SPINNER_LABELS;

pub enum Screen {
    BugList,
    BugEditing,
}

pub enum ActivePanel {
    Left,
    Right,
}

/// Represents a row in our "bugs" table
pub struct Bug {
    pub bug_id: u32,
    pub date: String,
    pub title: String,
    pub description: String,
}

/// Represents the state of the TUI application.
pub struct App {
    pub table_items: Vec<Bug>,
    pub table_state: TableState,
    pub scrollbar_state: ScrollbarState,
    pub selected_bug_index: Option<usize>,
    pub active_panel: ActivePanel,
    pub current_screen: Screen,
    pub right_panel_scroll: u16,
    pub scroll_to_end: bool,
    pub gemini_response: Arc<Mutex<String>>,
    /// Whether the spinner in the bottom bar is enabled (toggled by 's')
    pub spinner_enabled: bool,
    /// Stateful state for spinner animation
    pub spinner_state: ThrobberState,
    /// Current index for the spinner label in SPINNER_LABELS
    pub spinner_label_index: usize,
}

impl App {
    /// Creates a new instance of the application with the initial state.
    pub fn new() -> App {
        let items = vec![
            Bug { bug_id: 1, date: "2025-08-01".to_string(), title: "Fix UI glitch on main screen".to_string(), description: "The main screen UI flickers when resizing the window. This seems to be related to the new rendering engine. We need to investigate the cause and apply a fix. The issue is most noticeable on high-resolution displays.".to_string() },
            Bug { bug_id: 2, date: "2025-08-02".to_string(), title: "Add support for new API endpoint".to_string(), description: "The application needs to integrate with the new 'v2/users' API endpoint. This involves creating a new service, updating the data models, and ensuring backward compatibility with the old endpoint.".to_string() },
            Bug { bug_id: 3, date: "2025-08-03".to_string(), title: "Improve database query performance".to_string(), description: "The dashboard is loading slowly due to an inefficient database query. We need to analyze the query, add the necessary indexes, and optimize the code to reduce the load time.".to_string() },
            Bug { bug_id: 4, date: "2025-08-04".to_string(), title: "User authentication fails with special characters".to_string(), description: "Users with special characters in their passwords are unable to log in. The issue seems to be in the password hashing function. We need to update the function to correctly handle all special characters.".to_string() },
            Bug { bug_id: 5, date: "2025-08-05".to_string(), title: "Crash on file upload with large files".to_string(), description: "The application crashes when a user tries to upload a file larger than 1GB. This is likely due to a memory allocation issue. We need to implement chunked file uploading to handle large files.".to_string() },
            Bug { bug_id: 6, date: "2025-08-06".to_string(), title: "Incorrect data displayed in the dashboard".to_string(), description: "The dashboard is showing incorrect sales figures for the last quarter. The issue is likely in the data aggregation logic. We need to review the code and fix the calculation.".to_string() },
            Bug { bug_id: 7, date: "2025-08-07".to_string(), title: "Button click does not trigger action".to_string(), description: "The 'Save' button in the settings menu is not working. The event handler is not being called. We need to investigate the cause and fix the event binding.".to_string() },
            Bug { bug_id: 8, date: "2025-08-08".to_string(), title: "Memory leak in the background service".to_string(), description: "The background service is consuming more and more memory over time, eventually leading to a crash. We need to use a memory profiler to identify the source of the leak and fix it.".to_string() },
            Bug { bug_id: 9, date: "2025-08-09".to_string(), title: "Text overlaps in the settings menu".to_string(), description: "The text in the settings menu overlaps on smaller screens. This is a CSS issue. We need to adjust the layout and styling to ensure the text is displayed correctly on all screen sizes.".to_string() },
            Bug { bug_id: 10, date: "2025-08-10".to_string(), title: "Application hangs on exit".to_string(), description: "The application does not exit cleanly and hangs for a few seconds before closing. This might be due to a thread not being properly terminated. We need to investigate the cause and ensure all threads are shut down gracefully.".to_string() },
            Bug { bug_id: 11, date: "2025-08-11".to_string(), title: "Search functionality returns irrelevant results".to_string(), description: "The search functionality is not returning the expected results. The scoring algorithm needs to be adjusted to give more weight to the title and less to the description.".to_string() },
            Bug { bug_id: 12, date: "2025-08-12".to_string(), title: "Export to CSV is not working".to_string(), description: "The 'Export to CSV' feature is failing with an error. The issue seems to be in the CSV generation library. We need to update the library or find an alternative.".to_string() },
            Bug { bug_id: 13, date: "2025-08-13".to_string(), title: "Email notifications are not being sent".to_string(), description: "The email notification service is not sending emails for new user registrations. The SMTP server configuration seems to be incorrect. We need to verify the settings and ensure the service is running correctly.".to_string() },
            Bug { bug_id: 14, date: "2025-08-14".to_string(), title: "Date format is incorrect in the report".to_string(), description: "The date format in the monthly report is incorrect. It should be 'YYYY-MM-DD' but is currently 'MM/DD/YYYY'. We need to update the date formatting logic.".to_string() },
            Bug { bug_id: 15, date: "2025-08-15".to_string(), title: "Sorting by date does not work as expected".to_string(), description: "When sorting the bug list by date, the order is incorrect. The sorting logic needs to be reviewed and fixed.".to_string() },
            Bug { bug_id: 16, date: "2025-08-16".to_string(), title: "UI does not update after deleting an item".to_string(), description: "After deleting a bug from the list, the UI is not updated to reflect the change. The application needs to be restarted to see the updated list. We need to implement a mechanism to refresh the UI after a deletion.".to_string() },
            Bug { bug_id: 17, date: "2025-08-17".to_string(), title: "API rate limit is too low".to_string(), description: "The application is hitting the API rate limit too frequently. We need to implement a caching mechanism to reduce the number of API calls.".to_string() },
            Bug { bug_id: 18, date: "2025-08-18".to_string(), title: "Scrollbar is not visible in the table".to_string(), description: "The scrollbar in the bug list table is not visible when there are more items than can be displayed. This is a CSS issue. We need to adjust the styling to make the scrollbar visible.".to_string() },
            Bug { bug_id: 19, date: "2025-08-19".to_string(), title: "Error message is not user-friendly".to_string(), description: "The error messages displayed to the user are too technical. We need to replace them with more user-friendly messages that explain the issue and suggest a solution.".to_string() },
            Bug { bug_id: 20, date: "2025-08-20".to_string(), title: "Application is not responsive on smaller screens".to_string(), description: "The application layout breaks on smaller screens. We need to implement a responsive design that adapts to different screen sizes.".to_string() },
            Bug { bug_id: 21, date: "2025-08-21".to_string(), title: "Incorrect permissions for new users".to_string(), description: "Newly registered users are being assigned incorrect permissions. The default permission set needs to be reviewed and corrected.".to_string() },
            Bug { bug_id: 22, date: "2025-08-22".to_string(), title: "Data corruption on saving".to_string(), description: "There are reports of data corruption when saving a bug report. This is a critical issue that needs to be investigated immediately. The data serialization logic is the most likely culprit.".to_string() },
            Bug { bug_id: 23, date: "2025-08-23".to_string(), title: "Login page is not mobile-friendly".to_string(), description: "The login page is difficult to use on mobile devices. The input fields are too small and the layout is not optimized for small screens. We need to create a mobile-friendly version of the login page.".to_string() },
            Bug { bug_id: 24, date: "2025-08-24".to_string(), title: "Session timeout is too short".to_string(), description: "Users are being logged out too frequently. The session timeout needs to be increased from 15 minutes to 1 hour.".to_string() },
            Bug { bug_id: 25, date: "2025-08-25".to_string(), title: "Missing translations for French language".to_string(), description: "Several UI elements are missing translations for the French language. We need to add the missing translations to the localization files.".to_string() },
            Bug { bug_id: 26, date: "2025-08-26".to_string(), title: "Password reset link expires too quickly".to_string(), description: "The password reset link expires in 10 minutes, which is too short for some users. We need to increase the expiration time to 1 hour.".to_string() },
            Bug { bug_id: 27, date: "2025-08-27".to_string(), title: "High CPU usage when idle".to_string(), description: "The application is consuming a high amount of CPU even when it's idle. This is likely due to a background process running in a tight loop. We need to identify the process and fix the issue.".to_string() },
            Bug { bug_id: 28, date: "2025-08-28".to_string(), title: "Cannot handle more than 100 concurrent users".to_string(), description: "The application crashes when there are more than 100 concurrent users. We need to perform load testing to identify the bottleneck and optimize the code to handle a higher number of users.".to_string() },
            Bug { bug_id: 29, date: "2025-08-29".to_string(), title: "Old data is not being archived properly".to_string(), description: "The data archiving process is not working correctly. Old bug reports are not being moved to the archive database. We need to investigate the issue and fix the archiving script.".to_string() },
            Bug { bug_id: 30, date: "2025-08-30".to_string(), title: "Security vulnerability in the login form".to_string(), description: "The login form is vulnerable to SQL injection. We need to update the code to use parameterized queries to prevent this vulnerability.".to_string() },
        ];
        let mut table_state = TableState::default();
        table_state.select(Some(0)); // Selects the first row by default
        let scrollbar_state = ScrollbarState::new(items.len());

        App {
            table_items: items,
            table_state,
            scrollbar_state,
            selected_bug_index: None,
            active_panel: ActivePanel::Left,
            current_screen: Screen::BugList,
            right_panel_scroll: 0,
            scroll_to_end: false,
            gemini_response: Arc::new(Mutex::new("Loading response from Gemini...".to_string())),
            spinner_enabled: false,
            spinner_state: ThrobberState::default(),
            spinner_label_index: 0,
        }
    }

    /// Moves the selection up in the table.
    pub fn previous_item(&mut self) {
        let i = match self.table_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.table_items.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.table_state.select(Some(i));
        self.scrollbar_state = self.scrollbar_state.position(i);
    }

    /// Moves the selection down in the table.
    pub fn next_item(&mut self) {
        let i = match self.table_state.selected() {
            Some(i) => {
                if i >= self.table_items.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.table_state.select(Some(i));
        self.scrollbar_state = self.scrollbar_state.position(i);
    }

    /// Toggles the spinner display in the bottom bar.
    pub fn toggle_spinner(&mut self) {
        self.spinner_enabled = !self.spinner_enabled;
        // Change the label with each 's' activation
        self.spinner_label_index = (self.spinner_label_index + 1) % SPINNER_LABELS.len();
    }

    pub fn page_up_item(&mut self) {
        let i = match self.table_state.selected() {
            Some(i) => i.saturating_sub(10),
            None => 0,
        };
        self.table_state.select(Some(i));
        self.scrollbar_state = self.scrollbar_state.position(i);
    }

    pub fn page_down_item(&mut self) {
        let i = match self.table_state.selected() {
            Some(i) => (i + 10).min(self.table_items.len() - 1),
            None => 0,
        };
        self.table_state.select(Some(i));
        self.scrollbar_state = self.scrollbar_state.position(i);
    }

    pub fn go_to_start(&mut self) {
        self.table_state.select(Some(0));
        self.scrollbar_state = self.scrollbar_state.position(0);
    }

    pub fn go_to_end(&mut self) {
        let i = self.table_items.len() - 1;
        self.table_state.select(Some(i));
        self.scrollbar_state = self.scrollbar_state.position(i);
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

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

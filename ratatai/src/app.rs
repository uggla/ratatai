// src/app.rs

use google_ai_rs::Client;
use launchpad_api_client::BugTaskEntry;
use ratatui::widgets::{ScrollbarState, TableState};
use std::sync::{Arc, Mutex};
use throbber_widgets_tui::ThrobberState;

use crate::ui::SPINNER_LABELS;

#[derive(Debug, PartialEq, Eq)]
pub enum Screen {
    BugList,
    BugEditing,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ActivePanel {
    Left,
    Right,
}

/// Represents a row in our "bugs" table
#[derive(Debug)]
pub struct Bug {
    pub bug_id: u32,
    pub date: String,
    pub title: String,
    pub description: String,
}

/// Represents the state of the TUI application.
#[derive(Debug)]
pub struct App {
    pub bug_table_items: Box<[BugTaskEntry]>,
    pub bug_table_state: TableState,
    pub bug_table_scrollbar_state: ScrollbarState,
    pub bug_table_selected_index: Option<usize>,
    pub active_panel: ActivePanel,
    pub current_screen: Screen,
    pub bug_desc_scroll: u16,
    pub bug_desc_scroll_to_end: bool,
    pub gemini_client: Arc<Client>,
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
    pub fn new(gemini_client: Client) -> App {
        let items = Box::new([]);
        let mut table_state = TableState::default();
        table_state.select(None);
        let scrollbar_state = ScrollbarState::new(0);

        App {
            bug_table_items: items,
            bug_table_state: table_state,
            bug_table_scrollbar_state: scrollbar_state,
            bug_table_selected_index: None,
            active_panel: ActivePanel::Left,
            current_screen: Screen::BugList,
            bug_desc_scroll: 0,
            bug_desc_scroll_to_end: false,
            gemini_client: Arc::new(gemini_client),
            gemini_response: Arc::new(Mutex::new("Loading response from Gemini...".to_string())),
            spinner_enabled: false,
            spinner_state: ThrobberState::default(),
            spinner_label_index: 0,
        }
    }

    /// Moves the selection up in the table.
    pub fn bug_table_previous_item(&mut self) {
        let i = match self.bug_table_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.bug_table_items.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.bug_table_state.select(Some(i));
        self.bug_table_scrollbar_state = self.bug_table_scrollbar_state.position(i);
    }

    /// Moves the selection down in the table.
    pub fn bug_table_next_item(&mut self) {
        let i = match self.bug_table_state.selected() {
            Some(i) => {
                if i >= self.bug_table_items.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.bug_table_state.select(Some(i));
        self.bug_table_scrollbar_state = self.bug_table_scrollbar_state.position(i);
    }

    pub fn bug_table_page_up_item(&mut self) {
        let i = match self.bug_table_state.selected() {
            Some(i) => i.saturating_sub(10),
            None => 0,
        };
        self.bug_table_state.select(Some(i));
        self.bug_table_scrollbar_state = self.bug_table_scrollbar_state.position(i);
    }

    pub fn bug_table_page_down_item(&mut self) {
        let i = match self.bug_table_state.selected() {
            Some(i) => (i + 10).min(self.bug_table_items.len() - 1),
            None => 0,
        };
        self.bug_table_state.select(Some(i));
        self.bug_table_scrollbar_state = self.bug_table_scrollbar_state.position(i);
    }

    pub fn bug_table_go_to_start(&mut self) {
        self.bug_table_state.select(Some(0));
        self.bug_table_scrollbar_state = self.bug_table_scrollbar_state.position(0);
    }

    pub fn bug_table_go_to_end(&mut self) {
        let i = self.bug_table_items.len() - 1;
        self.bug_table_state.select(Some(i));
        self.bug_table_scrollbar_state = self.bug_table_scrollbar_state.position(i);
    }

    /// Toggles the spinner display in the bottom bar.
    pub fn toggle_spinner(&mut self) {
        self.spinner_enabled = !self.spinner_enabled;
        // Change the label with each 's' activation
        self.spinner_label_index = (self.spinner_label_index + 1) % SPINNER_LABELS.len();
    }
}

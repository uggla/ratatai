// src/app.rs

use google_ai_rs::Client;
use launchpad_api_client::{
    BugTaskEntry, LaunchpadBug, StatusFilter, get_bug as lp_get_bug, get_project_bug_tasks,
};
use ratatui::widgets::{Cell, Row, ScrollbarState, TableState};
use regex::Regex;
use std::sync::{Arc, Mutex};
use throbber_widgets_tui::ThrobberState;
use tokio::sync::mpsc::Sender;
use tracing::{error, info};

use crate::{LpMessage, ui::SPINNER_LABELS};

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

/// Represents the state of the TUI application.
#[derive(Debug)]
pub struct App {
    pub bug_table_items: Box<[BugTaskEntry]>,
    pub bug_table_rows: Vec<Row<'static>>,
    pub bug_table_state: TableState,
    pub bug_table_scrollbar_state: ScrollbarState,
    pub active_panel: ActivePanel,
    pub current_screen: Screen,
    pub bug_desc_scroll: u16,
    pub bug_desc_scroll_to_end: bool,
    pub current_bug: Option<LaunchpadBug>,
    /// Whether the spinner in the bottom bar is enabled (toggled by 's')
    pub spinner_enabled: bool,
    /// Stateful state for spinner animation
    pub spinner_state: ThrobberState,
    /// Current index for the spinner label in SPINNER_LABELS
    pub spinner_label_index: usize,
    pub gemini_client: Arc<Client>,
    pub launchpad_client: Arc<launchpad_api_client::client::ReqwestClient>,
    pub gemini_response: Arc<Mutex<String>>,
    pub lp_sender: Sender<LpMessage>,
}

impl App {
    /// Creates a new instance of the application with the initial state.
    pub fn new(
        gemini_client: Client,
        launchpad_client: launchpad_api_client::client::ReqwestClient,
        lp_sender: Sender<LpMessage>,
    ) -> App {
        let items = Box::new([]);
        let mut table_state = TableState::default();
        table_state.select(None);
        let scrollbar_state = ScrollbarState::new(0);
        let rows = Vec::new();

        App {
            bug_table_items: items,
            bug_table_rows: rows,
            bug_table_state: table_state,
            bug_table_scrollbar_state: scrollbar_state,
            active_panel: ActivePanel::Left,
            current_screen: Screen::BugList,
            bug_desc_scroll: 0,
            bug_desc_scroll_to_end: false,
            current_bug: None,
            spinner_enabled: false,
            spinner_state: ThrobberState::default(),
            spinner_label_index: 0,
            gemini_client: Arc::new(gemini_client),
            launchpad_client: Arc::new(launchpad_client),
            gemini_response: Arc::new(Mutex::new("Loading response from Gemini...".to_string())),
            lp_sender,
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

    pub fn get_bugs(&mut self, project: String) {
        self.spinner_enabled = true;
        let sender = self.lp_sender.clone();
        let client = self.launchpad_client.clone();
        tokio::spawn(async move {
            info!("Task to get bugs started");

            match get_project_bug_tasks(&*client, &project, Some(StatusFilter::New)).await {
                Ok(mut bug_tasks) => {
                    bug_tasks.sort_by(|a, b| b.date_created.cmp(&a.date_created));

                    if let Err(e) = sender
                        .send(LpMessage::Bugs(bug_tasks.into_boxed_slice()))
                        .await
                    {
                        error!("Fail to send message, error {e}");
                    }
                }
                Err(e) => {
                    if let Err(e) = sender.send(LpMessage::Error(e)).await {
                        error!("Fail to send message, error {e}");
                    }
                }
            }
            info!("Task to get bugs completed");
        });
    }

    pub fn update_bugs(&mut self, bugs: Box<[BugTaskEntry]>, re: &Regex) {
        self.bug_table_items = bugs;
        self.bug_table_rows = self
            .bug_table_items
            .iter()
            .map(|item: &BugTaskEntry| {
                let height = 1;

                let extract_from_title = |o| -> (String, String) {
                    if let Some(caps) = re.captures(o) {
                        let id = &caps[1];
                        let title = &caps[2];
                        (id.to_string(), title.to_string())
                    } else {
                        ("".to_string(), "".to_string())
                    }
                };

                let (id, title) = extract_from_title(&item.title);

                let cells = vec![
                    Cell::from(id),
                    // I think we can unwrap safely as I guess we always have a date_created
                    Cell::from(item.date_created.unwrap().clone().date_naive().to_string()),
                    Cell::from(title),
                ];
                Row::new(cells).height(height as u16).bottom_margin(1)
            })
            .collect();
        self.bug_table_state.select(Some(0));
        self.bug_table_scrollbar_state = ScrollbarState::new(self.bug_table_items.len());
        self.spinner_enabled = false;
    }

    pub fn get_bug(&mut self, bug_id: u32) {
        self.spinner_enabled = true;
        let sender = self.lp_sender.clone();
        let client = self.launchpad_client.clone();
        tokio::spawn(async move {
            info!("Task to get bug started");

            match lp_get_bug(&*client, bug_id).await {
                Ok(bug) => {
                    if let Err(e) = sender.send(LpMessage::Bug(bug.into())).await {
                        error!("Fail to send message, error {e}");
                    }
                }
                Err(e) => {
                    if let Err(e) = sender.send(LpMessage::Error(e)).await {
                        error!("Fail to send message, error {e}");
                    }
                }
            }
            info!("Task to get bug completed");
        });
    }

    pub fn update_bug(&mut self, bug: LaunchpadBug) {
        self.current_bug = Some(bug);
        let mut response_guard = self.gemini_response.lock().unwrap();
        *response_guard = self.current_bug.as_ref().unwrap().description.clone();
        self.bug_desc_scroll = 0;
        self.bug_desc_scroll_to_end = false;
        self.spinner_enabled = false;
    }
}

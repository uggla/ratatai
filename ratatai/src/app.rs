// src/app.rs

use std::collections::HashSet;

use launchpad_api_client::{
    BugTaskEntry, LaunchpadBug, LaunchpadError, StatusFilter, get_bug as lp_get_bug,
    get_project_bug_tasks,
};
use ratatui::widgets::{Cell, Row, ScrollbarState, TableState};
use regex::Regex;
use std::sync::Arc;
use throbber_widgets_tui::ThrobberState;
use tokio::time::{Duration, sleep};
use tokio::{
    sync::mpsc::{self, Sender, UnboundedSender},
    task::JoinHandle,
};
use tracing::{error, info};

use crate::{LpMessage, ai_backend::AiProvider};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AiTarget {
    BugDescription,
    Draft,
}

pub(crate) struct AiResponse {
    pub generation: u64,
    pub target: AiTarget,
    pub result: anyhow::Result<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Screen {
    BugList,
    BugEditing,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ActivePanel {
    Left,
    Right,
}

/// Represents the state of the TUI application.
pub(crate) struct App {
    pub bug_table_items: Box<[BugTaskEntry]>,
    pub bug_table_rows: Vec<Row<'static>>,
    pub bug_table_state: TableState,
    pub bug_table_scrollbar_state: ScrollbarState,
    pub bug_list_message: Option<String>,
    pub active_panel: ActivePanel,
    pub current_screen: Screen,
    pub bug_desc_scroll: u16,
    pub bug_desc_scroll_to_end: bool,
    pub current_bug: Option<LaunchpadBug>,
    /// Whether the spinner in the bottom bar is enabled (toggled by 's')
    pub spinner_enabled: bool,
    /// Stateful state for spinner animation
    pub spinner_state: ThrobberState,
    loading_bug_list: bool,
    loading_bug: bool,
    ai_provider: Arc<dyn AiProvider>,
    ai_request_sender: Option<UnboundedSender<(AiTarget, String)>>,
    ai_worker: Option<JoinHandle<()>>,
    pending_ai_requests: usize,
    system_instruction: String,
    session_generation: u64,
    bug_request_generation: u64,
    ai_sender: Sender<AiResponse>,
    pub launchpad_client: Arc<launchpad_api_client::client::ReqwestClient>,
    pub bug_description_text: String,
    pub lp_sender: Sender<LpMessage>,
    pub bug_reply_text: String,
    /// Whether the first AI generation has been triggered in BugEditing mode
    pub ai_generation_triggered: bool,
    /// Bug IDs found in the triage roster. None if the roster could not be fetched.
    pub roster_bug_ids: Option<HashSet<u32>>,
}

impl App {
    /// Creates a new instance of the application with the initial state.
    pub(crate) fn new(
        ai_provider: Arc<dyn AiProvider>,
        system_instruction: String,
        launchpad_client: launchpad_api_client::client::ReqwestClient,
        lp_sender: Sender<LpMessage>,
        ai_sender: Sender<AiResponse>,
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
            bug_list_message: None,
            active_panel: ActivePanel::Left,
            current_screen: Screen::BugList,
            bug_desc_scroll: 0,
            bug_desc_scroll_to_end: false,
            current_bug: None,
            spinner_enabled: false,
            spinner_state: ThrobberState::default(),
            loading_bug_list: false,
            loading_bug: false,
            ai_provider,
            ai_request_sender: None,
            ai_worker: None,
            pending_ai_requests: 0,
            system_instruction,
            session_generation: 0,
            bug_request_generation: 0,
            ai_sender,
            launchpad_client: Arc::new(launchpad_client),
            bug_description_text: String::new(),
            lp_sender,
            bug_reply_text: String::new(),
            ai_generation_triggered: false,
            roster_bug_ids: None,
        }
    }

    /// Moves the selection up in the table.
    pub(crate) fn bug_table_previous_item(&mut self) {
        if self.bug_table_items.is_empty() {
            return;
        }

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
    pub(crate) fn bug_table_next_item(&mut self) {
        if self.bug_table_items.is_empty() {
            return;
        }

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

    pub(crate) fn bug_table_page_up_item(&mut self) {
        if self.bug_table_items.is_empty() {
            return;
        }

        let i = match self.bug_table_state.selected() {
            Some(i) => i.saturating_sub(10),
            None => 0,
        };
        self.bug_table_state.select(Some(i));
        self.bug_table_scrollbar_state = self.bug_table_scrollbar_state.position(i);
    }

    pub(crate) fn bug_table_page_down_item(&mut self) {
        if self.bug_table_items.is_empty() {
            return;
        }

        let i = match self.bug_table_state.selected() {
            Some(i) => (i + 10).min(self.bug_table_items.len() - 1),
            None => 0,
        };
        self.bug_table_state.select(Some(i));
        self.bug_table_scrollbar_state = self.bug_table_scrollbar_state.position(i);
    }

    pub(crate) fn bug_table_go_to_start(&mut self) {
        if self.bug_table_items.is_empty() {
            return;
        }

        self.bug_table_state.select(Some(0));
        self.bug_table_scrollbar_state = self.bug_table_scrollbar_state.position(0);
    }

    pub(crate) fn bug_table_go_to_end(&mut self) {
        if self.bug_table_items.is_empty() {
            return;
        }

        let i = self.bug_table_items.len() - 1;
        self.bug_table_state.select(Some(i));
        self.bug_table_scrollbar_state = self.bug_table_scrollbar_state.position(i);
    }

    /// Toggles the spinner display in the bottom bar.
    pub(crate) fn toggle_spinner(&mut self) {
        self.spinner_enabled = !self.spinner_enabled;
    }

    pub(crate) fn ai_provider_name(&self) -> &'static str {
        self.ai_provider.display_name()
    }

    pub(crate) fn is_loading(&self) -> bool {
        self.loading_bug_list || self.loading_bug || self.pending_ai_requests > 0
    }

    pub(crate) fn get_bugs(&mut self, project: String) {
        self.loading_bug_list = true;
        self.spinner_enabled = true;
        self.bug_list_message = None;
        let sender = self.lp_sender.clone();
        let client = self.launchpad_client.clone();
        tokio::spawn(async move {
            info!("Task to get bugs started");

            let mut timeout_count = 0;
            for attempt in 1..=2 {
                match get_project_bug_tasks(&*client, &project, Some(StatusFilter::New)).await {
                    Ok(mut bug_tasks) => {
                        bug_tasks.sort_by(|a, b| b.date_created.cmp(&a.date_created));

                        if let Err(e) = sender
                            .send(LpMessage::Bugs(bug_tasks.into_boxed_slice()))
                            .await
                        {
                            error!("Fail to send message, error {e}");
                        }
                        info!("Task to get bugs completed");
                        return;
                    }
                    Err(LaunchpadError::ApiTimeout { .. }) => {
                        timeout_count += 1;
                        if attempt < 2 {
                            sleep(Duration::from_millis(500)).await;
                        }
                    }
                    Err(e) => {
                        if let Err(e) = sender.send(LpMessage::Error(e)).await {
                            error!("Fail to send message, error {e}");
                        }
                        info!("Task to get bugs completed");
                        return;
                    }
                }
            }

            if timeout_count == 2
                && let Err(e) = sender
                    .send(LpMessage::BugListMessage(
                        "Launchpad timeout while refreshing bugs. Press 'r' to retry.".to_string(),
                    ))
                    .await
            {
                error!("Fail to send message, error {e}");
            }
            info!("Task to get bugs completed");
        });
    }

    pub(crate) fn update_bugs(&mut self, bugs: Box<[BugTaskEntry]>, re: &Regex) {
        self.bug_list_message = None;
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

                let roster_marker = match &self.roster_bug_ids {
                    None => "?",
                    Some(set) if id.parse::<u32>().is_ok_and(|n| set.contains(&n)) => "*",
                    Some(_) => "",
                };

                let cells = vec![
                    Cell::from(id),
                    // I think we can unwrap safely as I guess we always have a date_created
                    Cell::from(item.date_created.unwrap().clone().date_naive().to_string()),
                    Cell::from(roster_marker),
                    Cell::from(title),
                ];
                Row::new(cells).height(height as u16).bottom_margin(1)
            })
            .collect();
        self.bug_table_state.select(Some(0));
        self.bug_table_scrollbar_state = ScrollbarState::new(self.bug_table_items.len());
        self.loading_bug_list = false;
        self.spinner_enabled = self.is_loading();
    }

    pub(crate) fn update_bug_list_message(&mut self, message: String) {
        self.bug_table_items = Box::new([]);
        self.bug_table_rows.clear();
        self.bug_table_state.select(None);
        self.bug_table_scrollbar_state = ScrollbarState::new(0);
        self.bug_list_message = Some(message);
        self.loading_bug_list = false;
        self.spinner_enabled = self.is_loading();
    }

    pub(crate) fn get_bug(&mut self, bug_id: u32) {
        self.close_bug();
        self.loading_bug = true;
        self.spinner_enabled = true;
        let request_generation = self.bug_request_generation;
        let sender = self.lp_sender.clone();
        let client = self.launchpad_client.clone();
        tokio::spawn(async move {
            info!("Task to get bug started");

            match lp_get_bug(&*client, bug_id).await {
                Ok(bug) => {
                    if let Err(e) = sender
                        .send(LpMessage::Bug(request_generation, bug.into()))
                        .await
                    {
                        error!("Fail to send message, error {e}");
                    }
                }
                Err(e) => {
                    if let Err(e) = sender
                        .send(LpMessage::BugError(request_generation, e))
                        .await
                    {
                        error!("Fail to send message, error {e}");
                    }
                }
            }
            info!("Task to get bug completed");
        });
    }

    pub(crate) fn update_bug(&mut self, request_generation: u64, bug: LaunchpadBug) {
        if request_generation != self.bug_request_generation {
            return;
        }
        self.current_bug = Some(bug);
        self.bug_description_text = self.current_bug.as_ref().unwrap().description.clone();
        self.bug_reply_text.clear();
        self.ai_generation_triggered = false;
        self.bug_desc_scroll = 0;
        self.bug_desc_scroll_to_end = false;
        self.loading_bug = false;
        self.spinner_enabled = self.is_loading();
    }

    pub(crate) fn is_current_bug_request(&self, generation: u64) -> bool {
        generation == self.bug_request_generation
    }

    pub(crate) fn update_bug_reply(&mut self, msg: String) {
        self.bug_reply_text = msg;
    }

    pub(crate) fn close_bug(&mut self) {
        self.end_ai_session();
        self.loading_bug = false;
        self.spinner_enabled = self.is_loading();
        self.bug_request_generation += 1;
        self.current_bug = None;
        self.bug_description_text.clear();
        self.bug_reply_text.clear();
        self.ai_generation_triggered = false;
    }

    pub(crate) fn end_ai_session(&mut self) {
        self.session_generation += 1;
        self.ai_request_sender = None;
        if let Some(worker) = self.ai_worker.take() {
            worker.abort();
        }
        self.pending_ai_requests = 0;
        self.spinner_enabled = self.is_loading();
    }

    pub(crate) fn start_ai_session(&mut self) {
        self.end_ai_session();
        let mut session = self
            .ai_provider
            .start_session(self.system_instruction.clone());
        let (request_sender, mut request_receiver) = mpsc::unbounded_channel();
        let response_sender = self.ai_sender.clone();
        let generation = self.session_generation;
        self.ai_request_sender = Some(request_sender);
        self.ai_worker = Some(tokio::spawn(async move {
            while let Some((target, prompt)) = request_receiver.recv().await {
                let result = session.send(prompt).await;
                if response_sender
                    .send(AiResponse {
                        generation,
                        target,
                        result,
                    })
                    .await
                    .is_err()
                {
                    break;
                }
            }
        }));
    }

    pub(crate) fn request_ai(&mut self, target: AiTarget, prompt: String) {
        if let Some(sender) = &self.ai_request_sender {
            if sender.send((target, prompt)).is_ok() {
                self.pending_ai_requests += 1;
                self.spinner_enabled = true;
            }
        } else if target == AiTarget::BugDescription {
            let mut session = self
                .ai_provider
                .start_session(self.system_instruction.clone());
            let sender = self.ai_sender.clone();
            let generation = self.session_generation;
            self.pending_ai_requests += 1;
            self.spinner_enabled = true;
            tokio::spawn(async move {
                let result = session.send(prompt).await;
                let _ = sender
                    .send(AiResponse {
                        generation,
                        target,
                        result,
                    })
                    .await;
            });
        }
    }

    pub(crate) fn apply_ai_response(&mut self, response: AiResponse) {
        if response.generation != self.session_generation
            || (self.ai_request_sender.is_none()
                && (response.target == AiTarget::Draft || self.current_bug.is_none()))
        {
            return;
        }
        self.pending_ai_requests = self.pending_ai_requests.saturating_sub(1);
        let text = match response.result {
            Ok(text) => text,
            Err(e) => {
                error!("AI request failed: {e}");
                format!("⚠️ {e}")
            }
        };
        match response.target {
            AiTarget::BugDescription => self.bug_description_text = text,
            AiTarget::Draft => self.update_bug_reply(text),
        }
        self.spinner_enabled = self.is_loading();
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex as StdMutex};

    use async_trait::async_trait;
    use tokio::sync::mpsc;

    use super::*;
    use crate::ai_backend::AiSession;

    struct FakeProvider(Arc<StdMutex<Vec<Vec<String>>>>);

    impl AiProvider for FakeProvider {
        fn display_name(&self) -> &'static str {
            "Test AI"
        }

        fn start_session(&self, _instruction: String) -> Box<dyn AiSession> {
            let mut sessions = self.0.lock().unwrap();
            sessions.push(Vec::new());
            Box::new(FakeSession {
                sessions: Arc::clone(&self.0),
                index: sessions.len() - 1,
            })
        }
    }

    struct FakeSession {
        sessions: Arc<StdMutex<Vec<Vec<String>>>>,
        index: usize,
    }

    #[async_trait]
    impl AiSession for FakeSession {
        async fn send(&mut self, prompt: String) -> anyhow::Result<String> {
            self.sessions.lock().unwrap()[self.index].push(prompt.clone());
            Ok(prompt)
        }
    }

    #[tokio::test]
    async fn conversation_is_shared_then_reset_and_late_replies_are_ignored() {
        let sessions = Arc::new(StdMutex::new(Vec::new()));
        let (lp_sender, _) = mpsc::channel(1);
        let (ai_sender, mut ai_receiver) = mpsc::channel(4);
        let mut app = App::new(
            Arc::new(FakeProvider(Arc::clone(&sessions))),
            "triage rules".into(),
            launchpad_api_client::client::ReqwestClient::new(),
            lp_sender,
            ai_sender,
        );

        app.start_ai_session();
        app.request_ai(AiTarget::BugDescription, "analysis".into());
        app.request_ai(AiTarget::Draft, "draft".into());
        app.apply_ai_response(ai_receiver.recv().await.unwrap());
        app.apply_ai_response(ai_receiver.recv().await.unwrap());
        assert_eq!(sessions.lock().unwrap()[0], ["analysis", "draft"]);

        app.request_ai(AiTarget::Draft, "late".into());
        let late = ai_receiver.recv().await.unwrap();
        app.end_ai_session();
        app.apply_ai_response(late);
        assert_eq!(app.bug_reply_text, "draft");
        assert_eq!(app.bug_description_text, "analysis");

        app.start_ai_session();
        app.request_ai(AiTarget::Draft, "new".into());
        app.apply_ai_response(ai_receiver.recv().await.unwrap());
        let sessions = sessions.lock().unwrap();
        assert_eq!(sessions.len(), 2);
        assert_eq!(sessions[1], ["new"]);
        drop(sessions);
        app.close_bug();
        assert!(app.bug_description_text.is_empty());
    }
}

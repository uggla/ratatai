pub mod client;
mod fake;

use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::{Value, error::Category};
use thiserror::Error;
use tracing::debug;

#[derive(Error, Debug)]
pub enum LaunchpadError {
    #[error("HTTP request failed: {0}")]
    HttpRequest(#[from] reqwest::Error),
    #[error("Deserialization failed: {0}")]
    Deserialization(#[from] serde_json::Error),
    #[error("Invalid project: {0}")]
    InvalidProject(String),
}

pub trait HTTPClient {
    fn get(
        &self,
        url: &str,
    ) -> impl std::future::Future<Output = Result<String, LaunchpadError>> + Send;
}

const LAUNCHPAD_API_BASE_URL: &str = "https://api.launchpad.net/1.0";
const LAUNCHPAD_API_BUG_BASE_URL: &str = "https://api.launchpad.net/1.0/bugs";

#[derive(Debug)]
pub enum StatusFilter {
    New,
    Incomplete,
    Opinion,
    Invalid,
    WontFix,
    Confirmed,
    Triaged,
    InProgress,
    Deferred,
    FixCommitted,
    FixReleased,
}

impl From<StatusFilter> for String {
    fn from(value: StatusFilter) -> Self {
        match value {
            StatusFilter::New => String::from("New"),
            StatusFilter::Incomplete => String::from("Incomplete"),
            StatusFilter::Opinion => String::from("Opinion"),
            StatusFilter::Invalid => String::from("Invalid"),
            StatusFilter::WontFix => String::from("Won't+Fix"),
            StatusFilter::Confirmed => String::from("Confirmed"),
            StatusFilter::Triaged => String::from("Triaged"),
            StatusFilter::InProgress => String::from("In+Progress"),
            StatusFilter::Deferred => String::from("Deferred"),
            StatusFilter::FixCommitted => String::from("Fix+Committed"),
            StatusFilter::FixReleased => String::from("Fix+Released"),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct LaunchpadBugTasksResponse {
    pub start: u32,
    pub total_size: u32,
    pub next_collection_link: Option<String>,
    pub entries: Vec<BugTaskEntry>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct BugTaskEntry {
    pub self_link: String,
    pub web_link: String,
    pub resource_type_link: String,
    pub bug_link: String,
    pub milestone_link: Option<String>,
    pub status: String,
    pub importance: String,
    pub assignee_link: Option<String>,
    pub bug_target_display_name: String,
    pub bug_target_name: String,
    pub bug_watch_link: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_datetime")]
    pub date_assigned: Option<DateTime<Utc>>,
    #[serde(default, deserialize_with = "deserialize_optional_datetime")]
    pub date_created: Option<DateTime<Utc>>,
    #[serde(default, deserialize_with = "deserialize_optional_datetime")]
    pub date_confirmed: Option<DateTime<Utc>>,
    #[serde(default, deserialize_with = "deserialize_optional_datetime")]
    pub date_incomplete: Option<DateTime<Utc>>,
    #[serde(default, deserialize_with = "deserialize_optional_datetime")]
    pub date_in_progress: Option<DateTime<Utc>>,
    #[serde(default, deserialize_with = "deserialize_optional_datetime")]
    pub date_closed: Option<DateTime<Utc>>,
    #[serde(default, deserialize_with = "deserialize_optional_datetime")]
    pub date_left_new: Option<DateTime<Utc>>,
    #[serde(default, deserialize_with = "deserialize_optional_datetime")]
    pub date_triaged: Option<DateTime<Utc>>,
    #[serde(default, deserialize_with = "deserialize_optional_datetime")]
    pub date_fix_committed: Option<DateTime<Utc>>,
    #[serde(default, deserialize_with = "deserialize_optional_datetime")]
    pub date_fix_released: Option<DateTime<Utc>>,
    #[serde(default, deserialize_with = "deserialize_optional_datetime")]
    pub date_left_closed: Option<DateTime<Utc>>,
    #[serde(default, deserialize_with = "deserialize_optional_datetime")]
    pub date_deferred: Option<DateTime<Utc>>,
    pub owner_link: Option<String>,
    pub target_link: String,
    pub title: String,
    pub related_tasks_collection_link: String,
    pub is_complete: bool,
    pub http_etag: String,
}

#[derive(Debug, Deserialize)]
pub struct LaunchpadBug {
    pub self_link: String,
    pub web_link: String,
    pub resource_type_link: String,
    pub id: u32,
    pub private: bool,
    pub information_type: String,
    pub name: Option<String>,
    pub title: String,
    pub description: String,
    pub owner_link: String,
    pub bug_tasks_collection_link: String,
    pub duplicate_of_link: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_datetime")]
    pub date_created: Option<DateTime<Utc>>,
    pub activity_collection_link: String,
    pub can_expire: bool,
    pub subscriptions_collection_link: String,
    #[serde(default, deserialize_with = "deserialize_optional_datetime")]
    pub date_last_updated: Option<DateTime<Utc>>,
    pub who_made_private_link: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_datetime")]
    pub date_made_private: Option<DateTime<Utc>>, // Peut être null
    pub heat: u32,
    pub bug_watches_collection_link: String,
    pub cves_collection_link: String,
    pub vulnerabilities_collection_link: String,
    pub duplicates_collection_link: String,
    pub attachments_collection_link: String,
    pub security_related: bool,
    pub latest_patch_uploaded: Option<String>,
    pub tags: Vec<String>,
    #[serde(default, deserialize_with = "deserialize_optional_datetime")]
    pub date_last_message: Option<DateTime<Utc>>,
    pub number_of_duplicates: u32,
    pub message_count: u32,
    pub users_affected_count: u32,
    pub users_unaffected_count: u32,
    pub users_affected_collection_link: String,
    pub users_unaffected_collection_link: String,
    pub users_affected_count_with_dupes: u32,
    pub other_users_affected_count_with_dupes: u32,
    pub users_affected_with_dupes_collection_link: String,
    pub messages_collection_link: String,
    pub linked_branches_collection_link: String,
    pub http_etag: String,
}

// Helper function to deserialize optional date-time strings.
// Launchpad API dates are in ISO 8601 format, e.g., "2025-01-13T08:46:25.105013+00:00"
// Some date fields can be null or completely absent.
fn deserialize_optional_datetime<'de, D>(deserializer: D) -> Result<Option<DateTime<Utc>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Option::deserialize(deserializer).map(|opt_string: Option<String>| {
        opt_string.and_then(|s| {
            if s.is_empty() {
                // Handle cases where it might be ""
                None
            } else {
                s.parse::<DateTime<Utc>>().ok()
            }
        })
    })
}

pub async fn get_bug(
    client: &impl HTTPClient,
    bug_id: u32,
) -> Result<LaunchpadBug, LaunchpadError> {
    let url = format!("{LAUNCHPAD_API_BUG_BASE_URL}/{bug_id}");
    debug!("Connecting to \"{url}\"");
    let response = client.get(&url).await?;

    let bug: LaunchpadBug = serde_json::from_str(&response)?;
    Ok(bug)
}

pub async fn get_project_bug_tasks(
    client: &impl HTTPClient,
    project_name: &str,
    filter: Option<StatusFilter>,
) -> Result<Vec<BugTaskEntry>, LaunchpadError> {
    let url = format!("{LAUNCHPAD_API_BASE_URL}/{project_name}");
    debug!("Connecting to \"{url}\"");
    let response = client.get(&url).await?;

    check_project(project_name, &url, &response)?;

    // At this point we have a valid project
    let url = match filter {
        None => format!("{LAUNCHPAD_API_BASE_URL}/{project_name}?ws.op=searchTasks"),
        Some(f) => format!(
            "{LAUNCHPAD_API_BASE_URL}/{project_name}?ws.op=searchTasks&status={}",
            String::from(f)
        ),
    };

    let mut bug_tasks_page = get_bug_tasks_page(client, &url).await?;

    let mut bugtasks: Vec<BugTaskEntry> = Vec::with_capacity(bug_tasks_page.total_size as usize);

    copy_bug_tasks_page(&bug_tasks_page, &mut bugtasks);

    while bug_tasks_page.next_collection_link.is_some() {
        bug_tasks_page =
            get_bug_tasks_page(client, &bug_tasks_page.next_collection_link.unwrap()).await?;
        copy_bug_tasks_page(&bug_tasks_page, &mut bugtasks);
    }

    Ok(bugtasks)
}

fn check_project(project_name: &str, url: &str, response: &str) -> Result<Value, LaunchpadError> {
    let project: Result<Value, serde_json::Error> = serde_json::from_str(response);

    // Check for invalid json
    let project = match project {
        Ok(v) => v,
        Err(e) => match e.classify() {
            Category::Syntax => {
                return Err(LaunchpadError::InvalidProject(project_name.to_string()));
            }
            _ => return Err(LaunchpadError::Deserialization(e)),
        },
    };

    // Check for valid json but wrong content
    if project["self_link"] != url {
        return Err(LaunchpadError::InvalidProject(project_name.to_string()));
    }

    Ok(project)
}

async fn get_bug_tasks_page(
    client: &impl HTTPClient,
    url: &str,
) -> Result<LaunchpadBugTasksResponse, LaunchpadError> {
    debug!("Connecting to \"{url}\"");
    let tasks_response_text = client.get(url).await?;
    let bug_tasks_response: LaunchpadBugTasksResponse = serde_json::from_str(&tasks_response_text)?;
    Ok(bug_tasks_response)
}

fn copy_bug_tasks_page(
    bug_tasks_response: &LaunchpadBugTasksResponse,
    bugtasks: &mut Vec<BugTaskEntry>,
) {
    bug_tasks_response
        .entries
        .iter()
        .for_each(|bt| bugtasks.push(bt.clone()));
}

#[cfg(test)]
mod tests {
    use crate::client::FakeClient;

    use super::*;

    #[tokio::test]
    async fn test_get_bug() {
        let client = FakeClient::new();

        let json = get_bug(&client, 666).await;

        assert!(json.is_ok());
        assert_eq!(json.unwrap().id, 666);
    }

    #[tokio::test]
    async fn test_get_bug_deserialize_error() {
        let client = FakeClient::new();

        let json = get_bug(&client, 5000).await;

        // assert that the result is an err
        assert!(json.is_err());

        let error = format!("{:?}", json.unwrap_err());
        assert_eq!(
            "Deserialization(Error(\"missing field `self_link`\", line: 45, column: 1))",
            &error
        );
    }

    #[tokio::test]
    async fn test_get_project_bugs() {
        let bug_links_ref = [
            "https://api.launchpad.net/1.0/bugs/2093869",
            "https://api.launchpad.net/1.0/bugs/2093879",
            "https://api.launchpad.net/1.0/bugs/2066206",
            "https://api.launchpad.net/1.0/bugs/2067081",
        ];

        let client = FakeClient::new();

        let bug_tasks = get_project_bug_tasks(&client, "nova", Some(StatusFilter::New)).await;

        assert!(bug_tasks.is_ok());
        let bug_tasks = bug_tasks.unwrap();
        assert_eq!(bug_tasks.len(), 4);

        let bug_links: Vec<&String> = bug_tasks.iter().map(|b| &b.bug_link).collect();
        assert_eq!(bug_links, bug_links_ref);
    }

    #[tokio::test]
    async fn test_get_project_bugs_empty_json_invalid_project_error() {
        let client = FakeClient::new();

        let json = get_project_bug_tasks(&client, "zorglub", None).await;

        // assert that the result is an err
        assert!(json.is_err());

        let error = format!("{:?}", json.unwrap_err());
        assert_eq!("InvalidProject(\"zorglub\")", &error);
    }

    #[tokio::test]
    async fn test_get_project_bugs_invalid_json_invalid_project_error() {
        let client = FakeClient::new();

        let json = get_project_bug_tasks(&client, "notaproject", None).await;

        // assert that the result is an err
        assert!(json.is_err());

        let error = format!("{:?}", json.unwrap_err());
        assert_eq!("InvalidProject(\"notaproject\")", &error);
    }
}

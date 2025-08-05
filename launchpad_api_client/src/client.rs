use crate::{
    HTTPClient, LaunchpadError,
    fake::{fake_bug, fake_bug_tasks_page_1, fake_bug_tasks_page_2, fake_project},
};
use reqwest::Client;
#[derive(Debug)]
pub struct ReqwestClient(Client);

#[derive(Debug)]
pub(crate) struct FakeClient;

#[allow(dead_code)]
impl ReqwestClient {
    pub fn new() -> Self {
        Self(reqwest::Client::new())
    }
}

impl Default for ReqwestClient {
    fn default() -> Self {
        Self::new()
    }
}

impl HTTPClient for ReqwestClient {
    async fn get(&self, url: &str) -> Result<String, LaunchpadError> {
        let res = &self.0.get(url).send().await?.text().await?;
        Ok(res.to_string())
    }
}

#[allow(dead_code)]
impl FakeClient {
    pub fn new() -> Self {
        Self
    }
}

impl Default for FakeClient {
    fn default() -> Self {
        Self::new()
    }
}

impl HTTPClient for FakeClient {
    async fn get(&self, url: &str) -> Result<String, LaunchpadError> {
        match url {
            "https://api.launchpad.net/1.0/bugs/5000" => {
                Ok(fake_bug(url).replace("self_link", "dself_link"))
            }
            "https://api.launchpad.net/1.0/nova" => Ok(fake_project()),
            "https://api.launchpad.net/1.0/nova?ws.op=searchTasks&status=New" => Ok(fake_bug_tasks_page_1()),
            "https://api.launchpad.net/1.0/nova?status=New&ws.op=searchTasks&ws.size=2&memo=2&ws.start=2" => Ok(fake_bug_tasks_page_2()),
            "https://api.launchpad.net/1.0/zorglub" => Ok("{}".to_string()),
            "https://api.launchpad.net/1.0/notaproject" => Ok("Object: <lp.systemhomes.WebServiceApplication object at 0x7f92e7ae4730>, name: 'notaproject'".to_string()),
            // "https://api.launchpad.net/1.0/notaproject" => Err(LaunchpadError::Deserialization(
            //     serde_json::from_str::<serde_json::Value>("invalid json").unwrap_err(),
            // )),
            _ => Ok(fake_bug(url)),
        }
    }
}

// --- Test Module ---
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_invalid_url_error() {
        let client = ReqwestClient::new();
        let invalid_url = "this is not a valid url";

        let result = client.get(invalid_url).await;

        // assert that the result is an err
        assert!(result.is_err());

        let error = format!("{:?}", result.unwrap_err());
        assert_eq!(
            "HttpRequest(reqwest::Error { kind: Builder, source: RelativeUrlWithoutBase })",
            &error
        );
    }

    #[tokio::test]
    async fn test_get_url_not_exist_error() {
        let client = ReqwestClient::new();
        let invalid_url = "http://thisdomaindoesnotexist";

        let result = client.get(invalid_url).await;

        // assert that the result is an err
        assert!(result.is_err());

        let error = format!("{:?}", result.unwrap_err());
        assert_eq!(
            "HttpRequest(reqwest::Error { kind: Request, url: \"http://thisdomaindoesnotexist/\", source: hyper_util::client::legacy::Error(Connect, ConnectError(\"dns error\", Custom { kind: Uncategorized, error: \"failed to lookup address information: Name or service not known\" })) })",
            &error
        );
    }
    #[tokio::test]
    async fn test_fake_client() {
        let client = FakeClient::new();
        let invalid_url = "http://truc/bidule";

        let result = client.get(invalid_url).await;

        assert!(result.is_ok());

        let result = format!("{:?}", result.unwrap());
        assert!(&result.contains("\\\"id\\\": bidule"));
        assert!(&result.contains("\\\"self_link\\\": \\\"http://truc/bidule\\\""));
    }
}

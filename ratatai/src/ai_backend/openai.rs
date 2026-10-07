use std::str::FromStr;

use anyhow::{Context, bail};
use async_openai::{Client, config::OpenAIConfig};
use async_trait::async_trait;
use serde_json::{Value, json};

use super::{AiProvider, AiSession};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ReasoningEffort {
    Low,
    Medium,
    High,
}

impl ReasoningEffort {
    fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }
}

impl FromStr for ReasoningEffort {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "low" => Ok(Self::Low),
            "medium" => Ok(Self::Medium),
            "high" => Ok(Self::High),
            _ => bail!("OPENAI_REASONING_EFFORT must be low, medium, or high"),
        }
    }
}

pub(super) struct OpenAiProvider {
    client: Client<OpenAIConfig>,
    model: String,
    reasoning_effort: ReasoningEffort,
}

impl OpenAiProvider {
    pub(super) fn new(api_key: String, model: String, reasoning_effort: ReasoningEffort) -> Self {
        Self {
            client: Client::with_config(OpenAIConfig::new().with_api_key(api_key)),
            model,
            reasoning_effort,
        }
    }
}

impl AiProvider for OpenAiProvider {
    fn display_name(&self) -> &'static str {
        "OpenAI"
    }

    fn start_session(&self, instruction: String) -> Box<dyn AiSession> {
        Box::new(OpenAiSession {
            client: self.client.clone(),
            model: self.model.clone(),
            reasoning_effort: self.reasoning_effort,
            instruction,
            history: Vec::new(),
        })
    }
}

struct OpenAiSession {
    client: Client<OpenAIConfig>,
    model: String,
    reasoning_effort: ReasoningEffort,
    instruction: String,
    history: Vec<Value>,
}

#[async_trait]
impl AiSession for OpenAiSession {
    async fn send(&mut self, prompt: String) -> anyhow::Result<String> {
        let mut input = self.history.clone();
        input.push(json!({ "role": "user", "content": prompt }));
        let request = json!({
            "model": self.model,
            "instructions": self.instruction,
            "input": input,
            "reasoning": { "effort": self.reasoning_effort.as_str() },
            "include": ["reasoning.encrypted_content"],
            "store": false,
        });

        let body: Value = self
            .client
            .responses()
            .create_byot(request)
            .await
            .map_err(|error| anyhow::anyhow!("OpenAI request failed: {error}"))?;
        if body.get("status").and_then(Value::as_str) != Some("completed") {
            bail!("OpenAI response did not complete");
        }

        let output = body
            .get("output")
            .and_then(Value::as_array)
            .context("OpenAI response has no output")?;
        let text = output
            .iter()
            .filter(|item| item.get("type").and_then(Value::as_str) == Some("message"))
            .filter_map(|item| item.get("content").and_then(Value::as_array))
            .flatten()
            .filter(|part| part.get("type").and_then(Value::as_str) == Some("output_text"))
            .filter_map(|part| part.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join("\n");
        if text.trim().is_empty() {
            bail!("OpenAI returned no text");
        }

        input.extend(output.iter().cloned());
        self.history = input;
        Ok(text)
    }
}

#[cfg(test)]
mod tests {
    use async_openai::{Client, config::OpenAIConfig};
    use mockito::{Matcher, Server};
    use serde_json::json;

    use super::{AiProvider, OpenAiProvider, ReasoningEffort};

    #[tokio::test]
    async fn session_replays_all_output_items_without_storing_responses() {
        let mut server = Server::new_async().await;
        let mut provider =
            OpenAiProvider::new("test-key".into(), "test-model".into(), ReasoningEffort::Low);
        provider.client = Client::with_config(
            OpenAIConfig::new()
                .with_api_key("test-key")
                .with_api_base(format!("{}/v1", server.url())),
        );
        let mut session = provider.start_session("system instruction".into());

        let reasoning = json!({
            "type": "reasoning",
            "id": "rs_1",
            "encrypted_content": "encrypted"
        });
        let answer = json!({
            "type": "message",
            "id": "msg_1",
            "role": "assistant",
            "status": "completed",
            "content": [{ "type": "output_text", "text": "First answer", "annotations": [] }]
        });
        let first = server
            .mock("POST", "/v1/responses")
            .match_header("authorization", "Bearer test-key")
            .match_body(Matcher::Json(json!({
                "model": "test-model",
                "instructions": "system instruction",
                "input": [{ "role": "user", "content": "first" }],
                "reasoning": { "effort": "low" },
                "include": ["reasoning.encrypted_content"],
                "store": false
            })))
            .with_status(200)
            .with_body(json!({ "status": "completed", "output": [reasoning, answer] }).to_string())
            .create_async()
            .await;

        assert_eq!(session.send("first".into()).await.unwrap(), "First answer");
        first.assert_async().await;

        let second = server
            .mock("POST", "/v1/responses")
            .match_body(Matcher::Json(json!({
                "model": "test-model",
                "instructions": "system instruction",
                "input": [
                    { "role": "user", "content": "first" },
                    reasoning,
                    answer,
                    { "role": "user", "content": "follow up" }
                ],
                "reasoning": { "effort": "low" },
                "include": ["reasoning.encrypted_content"],
                "store": false
            })))
            .with_status(200)
            .with_body(
                json!({
                    "status": "completed",
                    "output": [{
                        "type": "message",
                        "role": "assistant",
                        "content": [{ "type": "output_text", "text": "Second answer" }]
                    }]
                })
                .to_string(),
            )
            .create_async()
            .await;

        assert_eq!(
            session.send("follow up".into()).await.unwrap(),
            "Second answer"
        );
        second.assert_async().await;

        let fresh = server
            .mock("POST", "/v1/responses")
            .match_body(Matcher::Json(json!({
                "model": "test-model",
                "instructions": "system instruction",
                "input": [{ "role": "user", "content": "new session" }],
                "reasoning": { "effort": "low" },
                "include": ["reasoning.encrypted_content"],
                "store": false
            })))
            .with_status(200)
            .with_body(
                json!({
                    "status": "completed",
                    "output": [{
                        "type": "message",
                        "role": "assistant",
                        "content": [{ "type": "output_text", "text": "Fresh answer" }]
                    }]
                })
                .to_string(),
            )
            .create_async()
            .await;
        let mut new_session = provider.start_session("system instruction".into());
        assert_eq!(
            new_session.send("new session".into()).await.unwrap(),
            "Fresh answer"
        );
        fresh.assert_async().await;
    }

    #[tokio::test]
    async fn failed_response_does_not_advance_session_history() {
        let mut server = Server::new_async().await;
        let mut provider = OpenAiProvider::new(
            "test-key".into(),
            "test-model".into(),
            ReasoningEffort::Medium,
        );
        provider.client = Client::with_config(
            OpenAIConfig::new()
                .with_api_key("test-key")
                .with_api_base(format!("{}/v1", server.url())),
        );
        let mut session = provider.start_session("instructions".into());

        let failed = server
            .mock("POST", "/v1/responses")
            .match_body(Matcher::Json(json!({
                "model": "test-model",
                "instructions": "instructions",
                "input": [{ "role": "user", "content": "retry" }],
                "reasoning": { "effort": "medium" },
                "include": ["reasoning.encrypted_content"],
                "store": false
            })))
            .with_status(400)
            .with_body(json!({ "error": { "message": "Invalid request" } }).to_string())
            .expect(1)
            .create_async()
            .await;

        assert!(
            session
                .send("retry".into())
                .await
                .unwrap_err()
                .to_string()
                .contains("Invalid request")
        );
        failed.assert_async().await;

        let after_failure = server
            .mock("POST", "/v1/responses")
            .match_body(Matcher::Json(json!({
                "model": "test-model",
                "instructions": "instructions",
                "input": [{ "role": "user", "content": "after failure" }],
                "reasoning": { "effort": "medium" },
                "include": ["reasoning.encrypted_content"],
                "store": false
            })))
            .with_status(200)
            .with_body(
                json!({
                    "status": "completed",
                    "output": [{
                        "type": "message",
                        "role": "assistant",
                        "content": [{ "type": "output_text", "text": "Recovered" }]
                    }]
                })
                .to_string(),
            )
            .create_async()
            .await;
        assert_eq!(
            session.send("after failure".into()).await.unwrap(),
            "Recovered"
        );
        after_failure.assert_async().await;
    }
}

use std::sync::Arc;

use async_trait::async_trait;
use google_ai_rs::{Client, genai::Content};

use super::{AiProvider, AiSession};

pub(super) struct GeminiProvider {
    client: Arc<Client>,
    model: String,
}

impl GeminiProvider {
    pub(super) async fn new(api_key: String, model: String) -> anyhow::Result<Self> {
        Ok(Self {
            client: Arc::new(Client::new(api_key).await?),
            model,
        })
    }
}

impl AiProvider for GeminiProvider {
    fn start_session(&self, instruction: String) -> Box<dyn AiSession> {
        Box::new(GeminiSession {
            client: Arc::clone(&self.client),
            model: self.model.clone(),
            instruction,
            history: Vec::new(),
        })
    }
}

struct GeminiSession {
    client: Arc<Client>,
    model: String,
    instruction: String,
    history: Vec<Content>,
}

#[async_trait]
impl AiSession for GeminiSession {
    async fn send(&mut self, prompt: String) -> anyhow::Result<String> {
        let model = self
            .client
            .generative_model(&self.model)
            .with_system_instruction(self.instruction.clone());
        let mut contents = self.history.clone();
        contents.push(Content::user(prompt));
        let response = model
            .generate_content(contents.clone())
            .await
            .map_err(|e| anyhow::anyhow!("{}", extract_user_message(&e.to_string())))?;
        let text = response.to_text();
        if text.trim().is_empty() {
            anyhow::bail!("Gemini returned no text");
        }
        let content = response
            .candidates
            .first()
            .and_then(|candidate| candidate.content.clone())
            .ok_or_else(|| anyhow::anyhow!("Gemini returned no content"))?;
        contents.push(content);
        self.history = contents;
        Ok(text)
    }
}

fn extract_user_message(full: &str) -> &str {
    if let Some(start) = full.find("message: \"") {
        let msg_start = start + "message: \"".len();
        if let Some(end) = full[msg_start..].find('"') {
            return &full[msg_start..msg_start + end];
        }
    }
    full
}

#[cfg(test)]
mod tests {
    use super::extract_user_message;

    #[test]
    fn extracts_tonic_message() {
        assert_eq!(
            extract_user_message("status, message: \"Try later\", details"),
            "Try later"
        );
    }
}

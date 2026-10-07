mod gemini;

use std::{env, sync::Arc};

use anyhow::bail;
use async_trait::async_trait;

pub(crate) trait AiProvider: Send + Sync {
    fn start_session(&self, instruction: String) -> Box<dyn AiSession>;
}

#[async_trait]
pub(crate) trait AiSession: Send {
    async fn send(&mut self, prompt: String) -> anyhow::Result<String>;
}

#[derive(Debug, PartialEq, Eq)]
enum ProviderSettings {
    Gemini { api_key: String, model: String },
}

fn required_env(name: &str, get: &impl Fn(&str) -> Option<String>) -> anyhow::Result<String> {
    let value = get(name).ok_or_else(|| anyhow::anyhow!("{name} is required"))?;
    if value.trim().is_empty() {
        bail!("{name} must not be empty");
    }
    Ok(value)
}

fn provider_settings(get: &impl Fn(&str) -> Option<String>) -> anyhow::Result<ProviderSettings> {
    match required_env("AI_PROVIDER", get)?.as_str() {
        "gemini" => Ok(ProviderSettings::Gemini {
            api_key: required_env("GEMINI_API_KEY", get)?,
            model: required_env("GEMINI_MODEL", get)?,
        }),
        other => bail!("Unsupported AI_PROVIDER '{other}'; this version supports 'gemini'"),
    }
}

pub(crate) async fn configured_provider() -> anyhow::Result<Arc<dyn AiProvider>> {
    let settings = provider_settings(&|name| env::var(name).ok())?;
    match settings {
        ProviderSettings::Gemini { api_key, model } => {
            Ok(Arc::new(gemini::GeminiProvider::new(api_key, model).await?))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::{ProviderSettings, provider_settings};

    #[test]
    fn provider_and_selected_settings_are_required() {
        let mut vars = HashMap::<&str, &str>::new();
        let get = |name: &str| vars.get(name).map(|value| value.to_string());
        assert!(
            provider_settings(&get)
                .unwrap_err()
                .to_string()
                .contains("AI_PROVIDER")
        );
        vars.insert("AI_PROVIDER", "gemini");
        assert!(
            provider_settings(&|name| vars.get(name).map(|v| v.to_string()))
                .unwrap_err()
                .to_string()
                .contains("GEMINI_API_KEY")
        );
        vars.insert("GEMINI_API_KEY", "key");
        vars.insert("GEMINI_MODEL", "model");
        assert!(matches!(
            provider_settings(&|name| vars.get(name).map(|v| v.to_string())).unwrap(),
            ProviderSettings::Gemini { .. }
        ));
        vars.insert("AI_PROVIDER", "openai");
        assert!(provider_settings(&|name| vars.get(name).map(|v| v.to_string())).is_err());
    }
}

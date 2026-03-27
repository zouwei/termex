use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// A streaming chunk from an AI response.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiChunk {
    pub text: String,
    pub done: bool,
}

/// Common request parameters for AI calls.
#[derive(Debug, Clone)]
pub struct AiRequest {
    pub system_prompt: String,
    pub user_message: String,
    pub max_tokens: u32,
}

/// Error type for AI provider operations.
#[derive(Debug, thiserror::Error)]
pub enum AiError {
    #[error("HTTP error: {0}")]
    Http(String),

    #[error("provider not configured")]
    NotConfigured,

    #[error("API error: {0}")]
    Api(String),

    #[error("parse error: {0}")]
    Parse(String),
}

/// AI provider trait — implemented by each backend (Claude, OpenAI, Ollama).
#[async_trait]
pub trait AiProvider: Send + Sync {
    /// Returns the provider name for display.
    fn name(&self) -> &str;

    /// Sends a request and returns the full response.
    async fn complete(&self, request: &AiRequest) -> Result<String, AiError>;

    /// Sends a request and streams response chunks via a callback.
    async fn stream(
        &self,
        request: &AiRequest,
        on_chunk: Box<dyn Fn(AiChunk) + Send>,
    ) -> Result<(), AiError>;
}

/// Configuration needed to create a provider instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderConfig {
    pub provider_type: String,
    pub api_key: String,
    pub api_base_url: Option<String>,
    pub model: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ai_chunk_serialize() {
        let chunk = AiChunk {
            text: "hello".into(),
            done: false,
        };
        let json = serde_json::to_string(&chunk).unwrap();
        assert!(json.contains("\"text\":\"hello\""));
        assert!(json.contains("\"done\":false"));
    }

    #[test]
    fn test_provider_config_roundtrip() {
        let config = ProviderConfig {
            provider_type: "openai".into(),
            api_key: "sk-test".into(),
            api_base_url: None,
            model: "gpt-4".into(),
        };
        let json = serde_json::to_string(&config).unwrap();
        let parsed: ProviderConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.provider_type, "openai");
        assert_eq!(parsed.model, "gpt-4");
    }
}

//! MClaw Gateway provider.
//!
//! This provider connects to a remote MClaw gateway server for LLM access.

use crate::providers::traits::{ChatMessage, Provider};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// MClaw gateway provider configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MClawGatewayConfig {
    /// Gateway URL (e.g., "http://gateway-server:42618")
    pub gateway_url: String,

    /// Client ID for authentication
    pub client_id: String,

    /// Client secret for authentication
    pub client_secret: String,
}

/// MClaw gateway provider.
pub struct MClawGatewayProvider {
    config: MClawGatewayConfig,
    client: Client,
}

impl MClawGatewayProvider {
    pub fn new(config: MClawGatewayConfig) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(120))
            .build()
            .unwrap();

        Self { config, client }
    }
}

#[async_trait]
impl Provider for MClawGatewayProvider {
    async fn chat_with_system(
        &self,
        _system_prompt: Option<&str>,
        message: &str,
        _model: &str,
        _temperature: f64,
    ) -> anyhow::Result<String> {
        let messages = vec![ChatMessage::user(message)];
        let request = ChatRequestPayload {
            messages: &messages,
            temperature: None,
            max_tokens: None,
        };

        let response = self.send_request(&request).await?;
        Ok(response.content)
    }

    async fn chat_with_history(
        &self,
        messages: &[ChatMessage],
        _model: &str,
        _temperature: f64,
    ) -> anyhow::Result<String> {
        let request = ChatRequestPayload {
            messages,
            temperature: None,
            max_tokens: None,
        };

        let response = self.send_request(&request).await?;
        Ok(response.content)
    }
}

impl MClawGatewayProvider {
    async fn send_request(&self, payload: &ChatRequestPayload<'_>) -> anyhow::Result<ChatResponsePayload> {
        let auth_token = format!(
            "mc_{}_{}",
            self.config.client_id, self.config.client_secret
        );

        let http_response = self
            .client
            .post(format!("{}/api/v1/chat", self.config.gateway_url))
            .header("Authorization", format!("Bearer {}", auth_token))
            .header("X-Client-Id", self.config.client_id.clone())
            .json(payload)
            .send()
            .await?;

        let status = http_response.status();
        let response_text = http_response.text().await?;

        if !status.is_success() {
            if let Ok(error_resp) = serde_json::from_str::<ErrorResponse>(&response_text) {
                anyhow::bail!("Gateway error: {}", error_resp.error);
            }
            anyhow::bail!("Gateway error {}: {}", status, response_text);
        }

        let chat_response: ChatResponsePayload = serde_json::from_str(&response_text)?;
        Ok(chat_response)
    }
}

#[derive(Debug, Serialize)]
struct ChatRequestPayload<'a> {
    messages: &'a [ChatMessage],
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct ChatResponsePayload {
    content: String,
    model: String,
    #[serde(default)]
    usage: Option<UsagePayload>,
}

#[derive(Debug, Deserialize)]
struct UsagePayload {
    #[serde(default)]
    input_tokens: Option<u64>,
    #[serde(default)]
    output_tokens: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct ErrorResponse {
    error: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_serialization() {
        let config = MClawGatewayConfig {
            gateway_url: "http://localhost:42618".to_string(),
            client_id: "test-group".to_string(),
            client_secret: "test-secret".to_string(),
        };

        let toml = toml::to_string(&config).unwrap();
        assert!(toml.contains("gateway_url"));
        assert!(toml.contains("client_id"));
    }
}

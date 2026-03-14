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

impl MClawGatewayConfig {
    /// Create configuration from config file or environment variables.
    ///
    /// Priority order:
    /// 1. Environment variables (MCLAW_GATEWAY_URL, MCLAW_CLIENT_ID, MCLAW_CLIENT_SECRET)
    /// 2. Config file (~/.mclaw/mclaw_provider.toml)
    /// 3. Defaults
    pub fn from_config_or_env(
        default_url: Option<String>,
        fallback_key: Option<&str>,
    ) -> Self {
        // First try environment variables - check if all are present
        let has_all_env_vars = std::env::var("MCLAW_GATEWAY_URL").is_ok()
            && std::env::var("MCLAW_CLIENT_ID").is_ok()
            && std::env::var("MCLAW_CLIENT_SECRET").is_ok();

        if has_all_env_vars {
            return Self {
                gateway_url: std::env::var("MCLAW_GATEWAY_URL").unwrap(),
                client_id: std::env::var("MCLAW_CLIENT_ID").unwrap(),
                client_secret: std::env::var("MCLAW_CLIENT_SECRET").unwrap(),
            };
        }

        // Try loading from config file
        if let Some(config) = Self::load_from_file() {
            return config;
        }

        // Fall back to defaults
        Self {
            gateway_url: default_url.unwrap_or_else(|| "http://localhost:42618".to_string()),
            client_id: std::env::var("MCLAW_CLIENT_ID")
                .unwrap_or_else(|_| "default".to_string()),
            client_secret: std::env::var("MCLAW_CLIENT_SECRET")
                .ok()
                .or_else(|| fallback_key.map(String::from))
                .unwrap_or_default(),
        }
    }

    /// Load configuration from ~/.mclaw/mclaw_provider.toml
    fn load_from_file() -> Option<Self> {
        let home = std::env::var("HOME").ok()?;
        let config_path = format!("{}/.mclaw/mclaw_provider.toml", home);

        let content = std::fs::read_to_string(&config_path).ok()?;

        toml::from_str(&content).ok()
    }
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

    /// Create provider from config file or environment variables.
    ///
    /// Priority order:
    /// 1. Environment variables (MCLAW_GATEWAY_URL, MCLAW_CLIENT_ID, MCLAW_CLIENT_SECRET)
    /// 2. Config file (~/.mclaw/mclaw_provider.toml)
    /// 3. Defaults
    pub fn from_config_or_env(default_url: Option<String>, fallback_key: Option<&str>) -> Self {
        Self::new(MClawGatewayConfig::from_config_or_env(default_url, fallback_key))
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

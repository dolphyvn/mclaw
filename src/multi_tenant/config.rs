//! Multi-tenant gateway configuration.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Multi-tenant gateway configuration.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MultiTenantConfig {
    /// Enable multi-tenant gateway mode
    #[serde(default)]
    pub enabled: bool,

    /// Host to bind to (default: 0.0.0.0)
    #[serde(default)]
    pub host: Option<String>,

    /// Port to bind to (default: 42618)
    #[serde(default)]
    pub port: Option<u16>,

    /// Client group configurations
    #[serde(default)]
    pub groups: HashMap<String, ClientGroup>,
}

impl Default for MultiTenantConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            host: None,
            port: None,
            groups: HashMap::new(),
        }
    }
}

/// Configuration for a single client group.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ClientGroup {
    /// Client ID (unique identifier for this group)
    #[serde(default)]
    pub client_id: String,

    /// Hashed client secret for authentication
    pub client_secret: String,

    /// Provider to use for this group (openrouter, openai, openai-codex, anthropic, glm, ollama, etc.)
    pub provider: String,

    /// Default model to use
    pub model: String,

    /// API key for this provider (required for most providers, optional for OAuth-based providers)
    #[serde(default)]
    pub api_key: String,

    /// Optional: OAuth auth profile name (for providers like openai-codex, copilot, gemini-oauth, etc.)
    #[serde(default)]
    pub auth_profile: Option<String>,

    /// Optional: API base URL for custom endpoints
    #[serde(default)]
    pub api_url: Option<String>,

    /// Optional: override temperature
    #[serde(default)]
    pub temperature: Option<f64>,

    /// Optional: max tokens limit
    #[serde(default)]
    pub max_tokens: Option<u32>,

    /// Optional: rate limit (requests per minute)
    #[serde(default)]
    pub rate_limit: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = MultiTenantConfig::default();
        assert!(!config.enabled);
        assert!(config.groups.is_empty());
    }

    #[test]
    fn test_client_group_serialization() {
        let group = ClientGroup {
            client_id: "test-group".to_string(),
            client_secret: "test-secret".to_string(),
            provider: "openrouter".to_string(),
            model: "test-model".to_string(),
            api_key: "test-key".to_string(),
            api_url: Some("https://example.com".to_string()),
            temperature: Some(0.7),
            max_tokens: Some(4096),
            rate_limit: Some(60),
        };

        let toml = toml::to_string(&group).unwrap();
        assert!(toml.contains("client_id"));
        assert!(toml.contains("provider"));
        assert!(toml.contains("0.7"));
    }
}

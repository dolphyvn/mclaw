//! WebSocket client for connecting to MClaw instances.

use super::config::MachineConfig;
use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::json;
use std::time::Duration;

/// WebSocket message types from MClaw.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
enum WsMessage {
    #[serde(rename = "chunk")]
    Chunk { content: String },
    #[serde(rename = "tool_call")]
    ToolCall { name: String, args: serde_json::Value },
    #[serde(rename = "tool_result")]
    ToolResult { name: String, output: Option<String> },
    #[serde(rename = "done")]
    Done { full_response: String },
    #[serde(rename = "error")]
    Error { message: String },
}

/// Client for communicating with an MClaw instance.
pub struct MClawClient {
    machine_name: String,
    url: String,
    token: Option<String>,
}

impl MClawClient {
    /// Create a new client from machine config.
    pub fn from_config(config: &MachineConfig) -> Self {
        Self {
            machine_name: config.name.clone(),
            url: config.url.clone(),
            token: config.token.clone(),
        }
    }

    /// Send a command and get the response.
    pub async fn send_command(&self, command: &str) -> Result<String> {
        // Build WebSocket URL with token if provided
        // Convert http:// to ws:// and https:// to wss://
        let base_url = self.url
            .replace("http://", "ws://")
            .replace("https://", "wss://");

        let ws_url = if let Some(token) = &self.token {
            format!("{}/ws/chat?token={}", base_url, token)
        } else {
            format!("{}/ws/chat", base_url)
        };

        tracing::debug!("Connecting to {} for machine {}", ws_url, self.machine_name);

        // Connect with timeout
        let (ws_stream, _) = tokio_tungstenite::connect_async(&ws_url)
            .await
            .with_context(|| format!("Failed to connect to {}", ws_url))?;

        let (mut sender, mut receiver) = ws_stream.split();

        // Send the command
        let request = json!({
            "type": "message",
            "content": command
        });

        let msg = tokio_tungstenite::tungstenite::Message::Text(request.to_string().into());
        sender
            .send(msg)
            .await
            .context("Failed to send command")?;

        // Collect response chunks
        let mut full_response = String::new();
        let mut tool_calls: Vec<String> = Vec::new();

        // Read messages with timeout
        let timeout_duration = Duration::from_secs(60);
        let start = std::time::Instant::now();

        while start.elapsed() < timeout_duration {
            match tokio::time::timeout(Duration::from_secs(5), receiver.next()).await {
                Ok(Some(Ok(msg))) => {
                    match msg {
                        tokio_tungstenite::tungstenite::Message::Text(text) => {
                            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&text) {
                                if let Some(msg_type) = parsed.get("type").and_then(|v| v.as_str()) {
                                    match msg_type {
                                        "chunk" => {
                                            if let Some(content) = parsed.get("content").and_then(|v| v.as_str()) {
                                                full_response.push_str(content);
                                            }
                                        }
                                        "done" => {
                                            if let Some(response) = parsed.get("full_response").and_then(|v| v.as_str()) {
                                                full_response = response.to_string();
                                            }
                                            return Ok(full_response);
                                        }
                                        "error" => {
                                            if let Some(message) = parsed.get("message").and_then(|v| v.as_str()) {
                                                return Ok(format!("Error: {}", message));
                                            }
                                        }
                                        "tool_call" => {
                                            if let Some(name) = parsed.get("name").and_then(|v| v.as_str()) {
                                                tool_calls.push(name.to_string());
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        }
                        tokio_tungstenite::tungstenite::Message::Close(_) => {
                            break;
                        }
                        _ => {}
                    }
                }
                Ok(None) => break,
                Ok(Some(Err(_))) => break,
                Err(_) => continue, // Timeout, keep trying
            }
        }

        // If we got chunks but no done message, return what we have
        if !full_response.is_empty() {
            return Ok(full_response);
        }

        // If we got tool calls, acknowledge them
        if !tool_calls.is_empty() {
            return Ok(format!("Executed: {}", tool_calls.join(", ")));
        }

        Ok("No response".to_string())
    }

    /// Check if the machine is reachable.
    pub async fn health_check(&self) -> Result<bool> {
        let health_url = format!("{}/health", self.url);

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()?;

        match client.get(&health_url).send().await {
            Ok(resp) => Ok(resp.status().is_success()),
            Err(_) => Ok(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_from_config() {
        let config = MachineConfig {
            name: "test".to_string(),
            url: "http://localhost:42618".to_string(),
            token: Some("token123".to_string()),
            default: false,
            description: None,
        };

        let client = MClawClient::from_config(&config);
        assert_eq!(client.machine_name, "test");
        assert_eq!(client.url, "http://localhost:42618");
        assert_eq!(client.token, Some("token123".to_string()));
    }
}

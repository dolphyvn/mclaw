//! Client-side WebSocket connector for dispatcher mode.
//!
//! When running in dispatcher mode, the gateway connects TO the dispatcher
//! via WebSocket, allowing NAT-friendly operation without requiring public IP.

use super::ws_server::WsClientMessage;
use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::interval;
use tokio_tungstenite::tungstenite::Message;

/// Configuration for dispatcher connector.
#[derive(Debug, Clone)]
pub struct ConnectorConfig {
    /// Dispatcher WebSocket URL (e.g., "ws://dispatcher:42619/ws/connect")
    pub ws_url: String,
    /// Machine name for this client.
    pub machine_name: String,
    /// Auth token for registration.
    pub auth_token: Option<String>,
    /// Reconnection interval in seconds.
    pub reconnect_interval_secs: u64,
}

impl Default for ConnectorConfig {
    fn default() -> Self {
        Self {
            ws_url: "ws://localhost:42619/ws/connect".to_string(),
            machine_name: hostname::get()
                .ok()
                .and_then(|h| h.into_string().ok())
                .unwrap_or_else(|| "mclaw-client".to_string()),
            auth_token: None,
            reconnect_interval_secs: 5,
        }
    }
}

/// Client-side dispatcher connector.
pub struct DispatcherConnector {
    config: ConnectorConfig,
    // Callback to execute commands
    executor: Arc<dyn CommandExecutor + Send + Sync>,
}

/// Trait for command execution.
#[async_trait::async_trait]
pub trait CommandExecutor: Send + Sync {
    async fn execute(&self, command: &str) -> Result<String>;
}

impl DispatcherConnector {
    /// Create a new connector.
    pub fn new(
        config: ConnectorConfig,
        executor: Arc<dyn CommandExecutor + Send + Sync>,
    ) -> Self {
        Self { config, executor }
    }

    /// Run the connector (blocks until shutdown).
    pub async fn run(&self) -> Result<()> {
        let ws_url = self.build_ws_url()?;
        let mut reconnect_interval = interval(Duration::from_secs(self.config.reconnect_interval_secs));

        loop {
            tracing::info!(
                "Connecting to dispatcher at {} as machine '{}'",
                ws_url,
                self.config.machine_name
            );

            match self.connect_and_run(&ws_url).await {
                Ok(_) => {
                    tracing::warn!("Dispatcher connection closed, will reconnect...");
                }
                Err(e) => {
                    tracing::error!("Dispatcher connection error: {}", e);
                }
            }

            reconnect_interval.tick().await;
        }
    }

    /// Build the WebSocket URL with query parameters.
    fn build_ws_url(&self) -> Result<String> {
        let mut url = self.config.ws_url.clone();
        if !url.contains('?') {
            url.push('?');
        } else {
            url.push('&');
        }
        url.push_str(&format!("machine_name={}", self.config.machine_name));
        if let Some(token) = &self.config.auth_token {
            url.push_str(&format!("&token={}", urlencoding::encode(token)));
        }
        Ok(url)
    }

    /// Connect and run the WebSocket loop.
    async fn connect_and_run(&self, ws_url: &str) -> Result<()> {
        let (ws_stream, _) = tokio_tungstenite::connect_async(ws_url)
            .await
            .with_context(|| format!("Failed to connect to dispatcher at {}", ws_url))?;

        tracing::info!("Connected to dispatcher");

        let (mut sender, mut receiver) = ws_stream.split();

        // Send initial registration message
        let register_msg = WsClientMessage::Register {
            machine_name: self.config.machine_name.clone(),
            auth_token: self.config.auth_token.clone(),
        };
        let register_json = serde_json::to_string(&register_msg)?;
        sender
            .send(Message::Text(register_json.into()))
            .await
            .context("Failed to send registration message")?;

        // Message loop with periodic ping
        let mut ping_interval = interval(Duration::from_secs(30));
        ping_interval.tick().await; // First tick completes immediately

        loop {
            tokio::select! {
                // Handle incoming messages
                msg = receiver.next() => {
                    match msg {
                        Some(Ok(Message::Text(text))) => {
                            if let Err(e) = self.handle_message(&mut sender, &text).await {
                                tracing::error!("Error handling message: {}", e);
                            }
                        }
                        Some(Ok(Message::Close(_))) => {
                            tracing::info!("Dispatcher closed connection");
                            break;
                        }
                        Some(Err(e)) => {
                            tracing::error!("WebSocket error: {}", e);
                            break;
                        }
                        None => {
                            tracing::info!("Dispatcher stream ended");
                            break;
                        }
                        Some(Ok(_)) => {}
                    }
                }
                // Send periodic ping
                _ = ping_interval.tick() => {
                    if let Err(e) = sender.send(Message::Text(json!({"type": "pong"}).to_string().into())).await {
                        tracing::warn!("Failed to send ping: {}", e);
                        break;
                    }
                }
            }
        }

        Ok(())
    }

    /// Handle an incoming message from dispatcher.
    async fn handle_message(
        &self,
        sender: &mut futures_util::stream::SplitSink<
            tokio_tungstenite::WebSocketStream<
                tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
            >,
            Message,
        >,
        text: &str,
    ) -> Result<()> {
        let value: serde_json::Value = serde_json::from_str(text)?;

        let msg_type = value
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        match msg_type {
            "command" => {
                let id = value
                    .get("id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Missing command id"))?;
                let command = value
                    .get("command")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Missing command field"))?;

                tracing::info!("Executing command from dispatcher: {}", command);

                let result = match self.executor.execute(command).await {
                    Ok(output) => WsClientMessage::Result {
                        id: id.to_string(),
                        output,
                        error: None,
                    },
                    Err(e) => {
                        let error_msg: String = e.to_string();
                        WsClientMessage::Result {
                            id: id.to_string(),
                            output: String::new(),
                            error: Some(error_msg),
                        }
                    }
                };

                let result_json = serde_json::to_string(&result)?;
                sender
                    .send(Message::Text(result_json.into()))
                    .await
                    .context("Failed to send result")?;
            }
            "ping" => {
                let pong = json!({"type": "pong"});
                sender
                    .send(Message::Text(pong.to_string().into()))
                    .await
                    .context("Failed to send pong")?;
            }
            _ => {
                tracing::warn!("Unknown message type: {}", msg_type);
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = ConnectorConfig::default();
        assert_eq!(config.reconnect_interval_secs, 5);
        assert!(!config.machine_name.is_empty());
    }

    #[test]
    fn test_build_ws_url() {
        let config = ConnectorConfig {
            ws_url: "ws://localhost:42619/ws/connect".to_string(),
            machine_name: "test-machine".to_string(),
            auth_token: Some("token123".to_string()),
            reconnect_interval_secs: 5,
        };

        let connector = DispatcherConnector::new(config, Arc::new(DummyExecutor));
        let url = connector.build_ws_url().unwrap();
        assert!(url.contains("machine_name=test-machine"));
        assert!(url.contains("token=token123"));
    }

    struct DummyExecutor;
    #[async_trait::async_trait]
    impl CommandExecutor for DummyExecutor {
        async fn execute(&self, _command: &str) -> Result<String> {
            Ok("ok".to_string())
        }
    }
}

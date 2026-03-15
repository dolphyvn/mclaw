//! Gateway dispatcher mode - client connects TO dispatcher via WebSocket.

use super::super::Config;
use anyhow::{Context, Result};
use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::interval;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::WebSocketStream;

/// Configuration for dispatcher connector.
#[derive(Debug, Clone)]
pub struct ConnectorConfig {
    /// Dispatcher WebSocket URL
    pub ws_url: String,
    /// Machine name for this client
    pub machine_name: String,
    /// Auth token for registration
    pub auth_token: Option<String>,
    /// Reconnection interval in seconds
    pub reconnect_interval_secs: u64,
}

/// Trait for command execution.
#[async_trait]
pub trait CommandExecutor: Send + Sync {
    async fn execute(&self, command: &str) -> Result<String>;
}

/// WebSocket message from server.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
enum WsServerMessage {
    #[serde(rename = "command")]
    Command { id: String, command: String },
    #[serde(rename = "ping")]
    Ping,
}

/// WebSocket message to server.
#[derive(Debug, Clone, Serialize)]
enum WsClientMessage {
    #[serde(rename = "result")]
    Result {
        id: String,
        output: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
    #[serde(rename = "pong")]
    Pong,
    #[serde(rename = "register")]
    Register {
        machine_name: String,
        auth_token: Option<String>,
    },
}

/// Dispatcher connector for gateway mode.
pub struct DispatcherConnector {
    config: ConnectorConfig,
    executor: Arc<dyn CommandExecutor + Send + Sync>,
}

impl DispatcherConnector {
    pub fn new(
        config: ConnectorConfig,
        executor: Arc<dyn CommandExecutor + Send + Sync>,
    ) -> Self {
        Self { config, executor }
    }

    pub async fn run(&self) -> Result<()> {
        let ws_url = self.config.ws_url.clone();
        let mut reconnect_interval = interval(Duration::from_secs(self.config.reconnect_interval_secs));

        tracing::info!("Dispatcher connector configured for: {}", ws_url);

        loop {
            tracing::info!(
                "Attempting connection to dispatcher at {} as machine '{}'",
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

    async fn connect_and_run(&self, ws_url: &str) -> Result<()> {
        tracing::debug!("Attempting WebSocket connection to: {}", ws_url);

        // Use connect_async - it should handle both ws:// and wss//
        let (ws_stream, response) = match tokio_tungstenite::connect_async(ws_url).await {
            Ok(result) => result,
            Err(e) => {
                tracing::error!("Connection error: {:?}", e);
                anyhow::bail!("Failed to connect to dispatcher at {}: {}", ws_url, e);
            }
        };

        tracing::info!("Connected to dispatcher, response: {:?}", response);

        type WsStream = WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;
        let ws_stream: WsStream = ws_stream;
        let (mut sender, mut receiver) = ws_stream.split::<Message>();

        // Send initial registration
        let register_msg = WsClientMessage::Register {
            machine_name: self.config.machine_name.clone(),
            auth_token: self.config.auth_token.clone(),
        };
        let register_json = serde_json::to_string(&register_msg)?;
        sender
            .send(Message::Text(register_json.into()))
            .await
            .context("Failed to send registration")?;

        // Message loop with periodic ping
        let mut ping_interval = interval(Duration::from_secs(30));
        ping_interval.tick().await;

        loop {
            tokio::select! {
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
                _ = ping_interval.tick() => {
                    if sender.send(Message::Text(serde_json::json!({"type": "pong"}).to_string().into())).await.is_err() {
                        break;
                    }
                }
            }
        }

        Ok(())
    }

    async fn handle_message(
        &self,
        sender: &mut futures_util::stream::SplitSink<
            WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
            Message,
        >,
        text: &str,
    ) -> Result<()> {
        let value: serde_json::Value = serde_json::from_str(text)?;
        let msg_type = value.get("type").and_then(|v| v.as_str()).unwrap_or("unknown");

        match msg_type {
            "command" => {
                let id = value.get("id").and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Missing command id"))?;
                let command = value.get("command").and_then(|v| v.as_str())
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
                sender.send(Message::Text(result_json.into())).await
                    .context("Failed to send result")?;
            }
            "ping" => {
                sender.send(Message::Text(serde_json::json!({"type": "pong"}).to_string().into())).await
                    .context("Failed to send pong")?;
            }
            _ => {}
        }

        Ok(())
    }
}

/// Run gateway in dispatcher client mode.
pub async fn run_gateway_dispatcher_mode(config: Config) -> Result<()> {
    let dispatcher_config = &config.dispatcher;

    let machine_name = dispatcher_config
        .machine_name
        .clone()
        .unwrap_or_else(|| hostname::get()
            .ok()
            .and_then(|h| h.into_string().ok())
            .unwrap_or_else(|| "mclaw-client".to_string()));

    let endpoint = build_dispatcher_endpoint(
        dispatcher_config,
        &machine_name,
        &dispatcher_config.auth_token,
    );

    let connector_config = ConnectorConfig {
        ws_url: endpoint.clone(),
        machine_name: machine_name.clone(),
        auth_token: dispatcher_config.auth_token.clone(),
        reconnect_interval_secs: dispatcher_config.reconnect_interval_secs,
    };

    let connector = DispatcherConnector::new(
        connector_config,
        Arc::new(GatewayCommandExecutor::new(config)),
    );

    tracing::info!(
        "🔗 Gateway starting in dispatcher mode as '{}' connecting to {}",
        machine_name,
        endpoint
    );

    connector.run().await
}

fn build_dispatcher_endpoint(
    dispatcher_config: &crate::config::schema::ClientDispatcherConfig,
    machine_name: &str,
    auth_token: &Option<String>,
) -> String {
    let base_url = if let Some(ep) = &dispatcher_config.endpoint {
        let ep = ep.trim_end_matches('/');
        // Convert HTTP to WebSocket
        let ws_url = if ep.starts_with("https://") {
            ep.replace("https://", "wss://")
        } else if ep.starts_with("http://") {
            ep.replace("http://", "ws://")
        } else {
            ep.to_string()
        };

        // Append /ws/connect if not present
        if ws_url.contains("/ws/connect") {
            ws_url
        } else {
            format!("{}/ws/connect", ws_url)
        }
    } else {
        "ws://localhost:42619/ws/connect".to_string()
    };

    // Add query parameters for authentication
    let mut url = base_url;
    if !url.contains('?') {
        url.push('?');
    } else {
        url.push('&');
    }
    url.push_str(&format!("machine_name={}", urlencode(machine_name)));

    if let Some(token) = auth_token {
        url.push('&');
        url.push_str(&format!("token={}", urlencode(token)));
    }

    url
}

fn urlencode(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
            _ => format!("%{:02X}", c as u8),
        })
        .collect()
}

/// Command executor that runs shell commands.
struct GatewayCommandExecutor {
    _config: Config,
}

impl GatewayCommandExecutor {
    fn new(config: Config) -> Self {
        Self { _config: config }
    }
}

#[async_trait]
impl CommandExecutor for GatewayCommandExecutor {
    async fn execute(&self, command: &str) -> Result<String> {
        use tokio::process::Command;
        let output = if cfg!(target_os = "windows") {
            Command::new("cmd").args(["/C", command]).output().await?
        } else {
            Command::new("sh").args(["-c", command]).output().await?
        };

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if output.status.success() {
            Ok(if stdout.is_empty() { stderr } else { stdout })
        } else {
            let error = if !stderr.is_empty() {
                stderr
            } else {
                format!("Command failed with exit code: {:?}", output.status.code())
            };
            Err(anyhow::anyhow!(error))
        }
    }
}

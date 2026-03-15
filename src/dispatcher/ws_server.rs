//! WebSocket server for reverse client connections.
//!
//! Clients connect TO the dispatcher via WebSocket, allowing NAT-friendly
//! command routing. The dispatcher maintains active connections and can
//! push commands through them instantly.

use super::config::MachineConfig;
use super::router::MachineResponse;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};

/// WebSocket message types.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WsServerMessage {
    #[serde(rename = "command")]
    Command { id: String, command: String },
    #[serde(rename = "ping")]
    Ping,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WsClientMessage {
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

/// Connected client info.
#[derive(Debug, Clone)]
struct ConnectedClient {
    machine_name: String,
    sender: mpsc::Sender<WsServerMessage>,
}

/// Registry of connected WebSocket clients.
pub struct ClientRegistry {
    clients: Arc<RwLock<HashMap<String, mpsc::Sender<WsServerMessage>>>>,
}

impl ClientRegistry {
    pub fn new() -> Self {
        Self {
            clients: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a client connection.
    pub async fn register(&self, machine_name: &str, sender: mpsc::Sender<WsServerMessage>) {
        let mut clients = self.clients.write().await;
        // Disconnect any existing connection for this machine
        if let Some(old) = clients.insert(machine_name.to_string(), sender) {
            let _ = old.send(WsServerMessage::Ping).await;
        }
        tracing::info!("WebSocket client registered: {}", machine_name);
    }

    /// Unregister a client connection.
    pub async fn unregister(&self, machine_name: &str) {
        let mut clients = self.clients.write().await;
        clients.remove(machine_name);
        tracing::info!("WebSocket client unregistered: {}", machine_name);
    }

    /// Check if a client is connected via WebSocket.
    pub async fn is_connected(&self, machine_name: &str) -> bool {
        let clients = self.clients.read().await;
        clients.contains_key(machine_name)
    }

    /// Send a command to a connected client.
    pub async fn send_command(
        &self,
        machine_name: &str,
        command_id: String,
        command: String,
    ) -> Result<()> {
        let clients = self.clients.read().await;
        if let Some(sender) = clients.get(machine_name) {
            sender
                .send(WsServerMessage::Command {
                    id: command_id,
                    command,
                })
                .await
                .context("Failed to send command to WebSocket client")?;
            Ok(())
        } else {
            anyhow::bail!("Client not connected via WebSocket: {}", machine_name)
        }
    }

    /// List all connected clients.
    pub async fn list_connected(&self) -> Vec<String> {
        let clients = self.clients.read().await;
        clients.keys().cloned().collect()
    }
}

/// In-flight command responses.
pub struct ResponseRegistry {
    responses: Arc<RwLock<HashMap<String, tokio::sync::oneshot::Sender<MachineResponse>>>>,
}

impl ResponseRegistry {
    pub fn new() -> Self {
        Self {
            responses: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a pending command and return a channel for the response.
    pub async fn register_pending(
        &self,
        command_id: String,
    ) -> tokio::sync::oneshot::Receiver<MachineResponse> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let mut responses = self.responses.write().await;
        responses.insert(command_id, tx);
        rx
    }

    /// Complete a pending command with its response.
    pub async fn complete(&self, command_id: String, response: MachineResponse) {
        let mut responses = self.responses.write().await;
        if let Some(tx) = responses.remove(&command_id) {
            let _ = tx.send(response);
        }
    }
}

/// Incoming WebSocket connection info.
#[derive(Debug, Deserialize)]
pub struct WsConnectQuery {
    pub machine_name: Option<String>,
    pub token: Option<String>,
}

/// Authenticate a connecting client.
pub fn authenticate_client(
    machine_name: &str,
    token: Option<&str>,
    machines: &[MachineConfig],
    _admin_token: Option<&str>,
) -> Result<()> {
    // Check if machine exists in registry
    let machine = machines
        .iter()
        .find(|m| m.name == machine_name)
        .ok_or_else(|| anyhow::anyhow!("Unknown machine: {}", machine_name))?;

    // Verify token if machine has one configured
    if let Some(machine_token) = &machine.token {
        let provided_token = token.unwrap_or("");
        if !constant_time_eq(machine_token, provided_token) {
            anyhow::bail!("Invalid token for machine: {}", machine_name);
        }
    }

    Ok(())
}

/// Constant-time comparison for tokens to prevent timing attacks.
fn constant_time_eq(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.bytes()
        .zip(b.bytes())
        .fold(0, |acc, (x, y)| acc | (x ^ y))
        == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constant_time_eq_same() {
        assert!(constant_time_eq("hello", "hello"));
    }

    #[test]
    fn test_constant_time_eq_different() {
        assert!(!constant_time_eq("hello", "world"));
    }

    #[test]
    fn test_constant_time_eq_different_length() {
        assert!(!constant_time_eq("hello", "hello!"));
    }

    #[tokio::test]
    async fn test_client_registry() {
        let registry = ClientRegistry::new();

        // Initially empty
        assert!(!registry.is_connected("test").await);

        // Register a client
        let (tx, _rx) = mpsc::channel(10);
        registry.register("test", tx).await;

        // Now connected
        assert!(registry.is_connected("test").await);
        assert_eq!(registry.list_connected().await, vec!["test".to_string()]);

        // Unregister
        registry.unregister("test").await;
        assert!(!registry.is_connected("test").await);
    }

    #[tokio::test]
    async fn test_response_registry() {
        let registry = ResponseRegistry::new();

        let id = "test-command".to_string();
        let mut rx = registry.register_pending(id.clone()).await;

        // Response not ready yet
        let timeout = tokio::time::timeout(
            tokio::time::Duration::from_millis(10),
            &mut rx,
        );
        assert!(timeout.await.is_err());

        // Complete the response
        let response = MachineResponse {
            machine: "test".to_string(),
            response: "output".to_string(),
            error: None,
        };
        registry.complete(id.clone(), response.clone()).await;

        // Now we can receive it
        let received = rx.await;
        assert_eq!(received.unwrap().response, "output");
    }
}

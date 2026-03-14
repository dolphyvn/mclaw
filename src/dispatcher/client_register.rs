//! Dispatcher client - handles registration with dispatcher service.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::time::interval;

/// Registration request sent to dispatcher.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DispatcherRegistration {
    pub machine_name: String,
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default)]
    pub default: bool,
}

/// Registration response from dispatcher.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct RegistrationResponse {
    status: String,
    machine: String,
    url: String,
}

/// Heartbeat request.
pub type HeartbeatRequest = DispatcherRegistration;

/// Dispatcher client for auto-registration.
#[derive(Debug, Clone)]
pub struct DispatcherClient {
    endpoint: String,
    registration: DispatcherRegistration,
    heartbeat_interval_secs: u64,
    client: reqwest::Client,
}

impl DispatcherClient {
    /// Create a new dispatcher client.
    pub fn new(
        endpoint: String,
        machine_name: String,
        url: String,
        auth_token: Option<String>,
        description: Option<String>,
        default: bool,
        heartbeat_interval_secs: u64,
    ) -> Self {
        let registration = DispatcherRegistration {
            machine_name,
            url,
            auth_token,
            description,
            default,
        };

        Self {
            endpoint,
            registration,
            heartbeat_interval_secs,
            client: reqwest::Client::new(),
        }
    }

    /// Register with the dispatcher.
    pub async fn register(&self) -> Result<()> {
        let url = format!("{}/register", self.endpoint);
        let response = self
            .client
            .post(&url)
            .json(&self.registration)
            .send()
            .await
            .with_context(|| format!("Failed to connect to dispatcher at {}", url))?;

        if response.status().is_success() {
            let resp: RegistrationResponse = response
                .json()
                .await
                .with_context(|| "Failed to parse registration response")?;
            tracing::info!(
                "Registered with dispatcher as '{}' at {}",
                resp.machine,
                resp.url
            );
            Ok(())
        } else {
            let status = response.status();
            let error = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            anyhow::bail!("Registration failed: {} - {}", status, error);
        }
    }

    /// Send heartbeat to dispatcher.
    pub async fn heartbeat(&self) -> Result<()> {
        let url = format!("{}/heartbeat", self.endpoint);
        let heartbeat = HeartbeatRequest {
            machine_name: self.registration.machine_name.clone(),
            url: self.registration.url.clone(),
            auth_token: None, // No auth needed for heartbeat
            description: None,
            default: false,
        };

        let response = self
            .client
            .post(&url)
            .json(&heartbeat)
            .send()
            .await
            .with_context(|| format!("Failed to send heartbeat to {}", url))?;

        if response.status().is_success() {
            tracing::debug!("Heartbeat sent to dispatcher");
            Ok(())
        } else {
            tracing::warn!("Heartbeat failed: {}", response.status());
            // Don't fail on heartbeat errors, just log them
            Ok(())
        }
    }

    /// Unregister from dispatcher.
    pub async fn unregister(&self) -> Result<()> {
        let url = format!("{}/unregister", self.endpoint);

        // Only include auth token if we have one
        let unregister_req = if self.registration.auth_token.is_some() {
            serde_json::to_value(self.registration.clone())?
        } else {
            serde_json::json!({
                "machine_name": self.registration.machine_name,
            })
        };

        let response = self
            .client
            .post(&url)
            .json(&unregister_req)
            .send()
            .await;

        if let Ok(resp) = response {
            if resp.status().is_success() {
                tracing::info!("Unregistered from dispatcher");
            } else {
                tracing::warn!("Unregister failed: {}", resp.status());
            }
        }

        Ok(())
    }

    /// Run the heartbeat loop in the background.
    pub fn spawn_heartbeat_task(self) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(self.heartbeat_interval_secs));
            loop {
                interval.tick().await;
                if let Err(e) = self.heartbeat().await {
                    tracing::debug!("Heartbeat error: {}", e);
                }
            }
        })
    }
}

/// Get the external URL for this machine.
pub fn get_external_url(port: u16) -> String {
    // Try to get the external IP, otherwise use hostname
    let hostname = hostname::get()
        .ok()
        .and_then(|h| h.into_string().ok())
        .unwrap_or_else(|| "localhost".to_string());

    // TODO: Add support for discovering actual external IP
    format!("http://{}:{}", hostname, port)
}

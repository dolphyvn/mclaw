//! MClaw Dispatcher - Multi-machine Telegram bot management service.
//!
//! The dispatcher receives Telegram messages, parses machine prefixes (@machine, @all),
//! routes commands to MClaw clients, and aggregates responses.

pub mod client;
pub mod client_register;
pub mod config;
pub mod machines;
pub mod router;
pub mod telegram;

// Re-export commonly used types
pub use client_register::DispatcherClient;

use self::config::ServiceConfig;
use self::machines::{MachineRegistry, RegistrationRequest};
use self::router::CommandRouter;
use self::telegram::{TelegramHandler, TelegramUpdate};
use anyhow::{Context, Result};
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::interval;

/// Shared application state.
#[derive(Clone)]
pub struct DispatcherState {
    pub config: Arc<ServiceConfig>,
    pub router: Arc<CommandRouter>,
    pub telegram: Arc<TelegramHandler>,
}

impl DispatcherState {
    /// Create a new state from configuration.
    pub fn new(config: ServiceConfig) -> Result<Self> {
        let registry = MachineRegistry::load(&config.machines.path)?;
        let router = Arc::new(CommandRouter::new(registry));
        let telegram = Arc::new(TelegramHandler::new(
            config.telegram.bot_token.clone(),
            (*router).clone(),
            config.telegram.allowed_users.clone(),
        ));

        Ok(Self {
            config: Arc::new(config),
            router,
            telegram,
        })
    }
}

/// Health check response.
#[derive(Debug, Serialize)]
struct HealthResponse {
    status: String,
    machines_count: usize,
}

/// Machines list response.
#[derive(Debug, Serialize)]
struct MachinesListResponse {
    machines: Vec<MachineInfo>,
}

#[derive(Debug, Serialize)]
struct MachineInfo {
    name: String,
    url: String,
    default: bool,
    description: Option<String>,
}

/// Dispatch request body.
#[derive(Debug, Deserialize)]
struct DispatchRequest {
    message: String,
}

/// Registration request body.
type RegistrationRequestBody = RegistrationRequest;

/// Unregister request body.
#[derive(Debug, Deserialize)]
struct UnregisterRequest {
    machine_name: String,
    auth_token: Option<String>,
}

/// Admin auth token from config.
#[derive(Debug, Clone)]
struct AdminAuth {
    token: String,
}

/// Run the dispatcher server.
pub async fn run_dispatcher(config: ServiceConfig) -> Result<()> {
    let state = DispatcherState::new(config.clone())?;

    // Build the router
    let app = Router::new()
        .route("/health", get(health_handler))
        .route("/machines", get(machines_handler))
        .route("/webhook", post(webhook_handler))
        .route("/webhook/telegram", post(webhook_handler))
        .route("/dispatch", post(dispatch_handler))
        .route("/register", post(register_handler))
        .route("/unregister", post(unregister_handler))
        .route("/heartbeat", post(heartbeat_handler))
        .route("/admin/machines", get(admin_machines_handler))
        .with_state(state.clone());

    // Set webhook if configured
    if let Some(webhook_url) = &config.telegram.webhook_url {
        if !webhook_url.is_empty() {
            tracing::info!("Setting Telegram webhook to: {}", webhook_url);
            if let Err(e) = state.telegram.set_webhook(webhook_url).await {
                tracing::warn!("Failed to set webhook: {}", e);
            }
        }
    }

    // Bind server
    let addr = format!("{}:{}", config.server.host, config.server.port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .with_context(|| format!("Failed to bind to {}", addr))?;

    let actual_addr = listener.local_addr()?;
    tracing::info!("Dispatcher server listening on {}", actual_addr);

    // Start server
    axum::serve(listener, app).await?;

    Ok(())
}

/// Health check handler.
async fn health_handler(State(state): State<DispatcherState>) -> impl IntoResponse {
    let machines_count = state.router.registry.list_names().len();

    Json(HealthResponse {
        status: "ok".to_string(),
        machines_count,
    })
}

/// Machines list handler.
async fn machines_handler(State(state): State<DispatcherState>) -> impl IntoResponse {
    let machines = state.router.registry.list_all();

    let response = MachinesListResponse {
        machines: machines
            .into_iter()
            .map(|m| MachineInfo {
                name: m.name,
                url: m.url,
                default: m.default,
                description: m.description,
            })
            .collect(),
    };

    Json(response)
}

/// Telegram webhook handler.
async fn webhook_handler(
    State(state): State<DispatcherState>,
    Json(update): Json<TelegramUpdate>,
) -> impl IntoResponse {
    tracing::debug!("Received webhook update: {}", update.update_id);

    // Handle the update asynchronously (don't block the response)
    let handler = state.telegram.clone();
    tokio::spawn(async move {
        if let Err(e) = handler.handle_update(update).await {
            tracing::error!("Error handling update: {}", e);
        }
    });

    (StatusCode::OK, "OK")
}

/// Dispatch endpoint for testing/alternative access.
async fn dispatch_handler(
    State(state): State<DispatcherState>,
    Json(req): Json<DispatchRequest>,
) -> impl IntoResponse {
    // Parse the command
    let parsed = match state.router.parse(&req.message) {
        Ok(p) => p,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": format!("Parse error: {}", e)
                })),
            ).into_response();
        }
    };

    // Execute the command
    match state.router.execute(&parsed).await {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("Execution error: {}", e)})),
        ).into_response(),
    }
}

/// Register a new machine (dynamic registration).
async fn register_handler(
    State(state): State<DispatcherState>,
    Json(req): Json<RegistrationRequestBody>,
) -> impl IntoResponse {
    match state.router.registry.register(
        req.clone(),
        if state.config.telegram.bot_token.is_empty() {
            None
        } else {
            Some(state.config.telegram.bot_token.as_str())
        } // Use bot token as admin token for now
    ) {
        Ok(()) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "status": "registered",
                "machine": req.machine_name,
                "url": req.url,
            })),
        ).into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": format!("Registration failed: {}", e)
            })),
        ).into_response(),
    }
}

/// Unregister a machine.
async fn unregister_handler(
    State(state): State<DispatcherState>,
    Json(req): Json<UnregisterRequest>,
) -> impl IntoResponse {
    match state.router.registry.unregister(
        &req.machine_name,
        req.auth_token.as_deref()
    ) {
        Ok(()) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "status": "unregistered",
                "machine": req.machine_name
            })),
        ).into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": format!("Unregistration failed: {}", e)
            })),
        ).into_response(),
    }
}

/// Heartbeat from a client.
async fn heartbeat_handler(
    State(state): State<DispatcherState>,
    Json(req): Json<RegistrationRequest>,
) -> impl IntoResponse {
    match state.router.registry.heartbeat(&req.machine_name) {
        Ok(true) => (StatusCode::OK, "OK").into_response(),
        Ok(false) => (
            StatusCode::NOT_FOUND,
            "Machine not found".to_string(),
        ).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Error: {}", e),
        ).into_response(),
    }
}

/// Admin endpoint to list all machines with health status.
async fn admin_machines_handler(
    State(state): State<DispatcherState>,
) -> impl IntoResponse {
    let health_status = state.router.registry.get_health_status();

    let machines: Vec<serde_json::Value> = state
        .router
        .registry
        .list_all()
        .into_iter()
        .map(|m| {
            let is_healthy = health_status
                .iter()
                .find(|(name, _)| name == &m.name)
                .map(|(_, healthy)| *healthy)
                .unwrap_or(true);

            serde_json::json!({
                "name": m.name,
                "url": m.url,
                "default": m.default,
                "description": m.description,
                "registered": is_healthy,  // true if has recent heartbeat
                "static": !m.token.is_none() || m.description.as_ref()
                    .map(|d| d.contains("(static)")).unwrap_or(false),
            })
        })
        .collect();

    Json(serde_json::json!({
        "machines": machines,
        "total": machines.len(),
    }))
}

/// Spawn background task to clean up stale machines.
pub fn spawn_cleanup_task(registry: MachineRegistry) {
    tokio::spawn(async move {
        let mut interval = interval(Duration::from_secs(30));
        loop {
            interval.tick().await;
            let stale = registry.remove_stale();
            if !stale.is_empty() {
                tracing::info!("Removed stale machines: {:?}", stale);
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_response_serialization() {
        let response = HealthResponse {
            status: "ok".to_string(),
            machines_count: 2,
        };
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"ok\""));
        assert!(json.contains("2"));
    }
}

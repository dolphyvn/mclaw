//! Multi-tenant gateway HTTP server.

use super::auth::AuthManager;
use base64::Engine;
use crate::config::Config;
use crate::providers::{ChatMessage, Provider};
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Json},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::signal;
use tower_http::cors::{Any, CorsLayer};
use tracing::{error, info, warn};

/// Application state for the gateway server.
#[derive(Clone)]
pub struct GatewayState {
    pub config: Config,
    pub providers: Arc<HashMap<String, Arc<dyn Provider>>>,
    pub auth: Arc<AuthManager>,
}

/// Run the multi-tenant gateway server.
pub async fn run_gateway_server(
    config: Config,
    host: String,
    port: u16,
) -> anyhow::Result<()> {
    // Initialize auth manager
    let auth = Arc::new(AuthManager::new());
    if let Some(multi_tenant) = &config.multi_tenant {
        auth.load_from_config(&multi_tenant.groups).await;
    }

    // Create provider instances for each unique provider+api_key combination
    let mut providers = HashMap::new();

    if let Some(multi_tenant) = &config.multi_tenant {
        for (_group_name, group) in &multi_tenant.groups {
            let provider_key = format!("{}:{}", group.provider, group.api_key.chars().take(4).collect::<String>());

            if !providers.contains_key(&provider_key) {
                let provider = create_provider(
                    &group.provider,
                    &group.api_key,
                    group.api_url.as_deref(),
                    &group.model,
                )?;
                providers.insert(provider_key, provider);
            }
        }
    }

    let state = GatewayState {
        config: config.clone(),
        providers: Arc::new(providers),
        auth,
    };

    // Build router
    let app = Router::new()
        .route("/health", get(health_check))
        .route("/api/v1/chat", post(handle_chat))
        .route("/api/v1/clients", get(list_clients))
        .layer(CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any))
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    info!("MClaw gateway listening on {}", addr);

    println!("🧠 MClaw Multi-Tenant Gateway");
    println!("   Host: {}", host);
    println!("   Port: {}", port);
    println!("   Health: http://{}:{}/health", host, port);
    println!("   Chat API: http://{}:{}/api/v1/chat", host, port);
    println!();

    if let Some(multi_tenant) = &config.multi_tenant {
        println!("   Configured clients: {}", multi_tenant.groups.len());
        for (name, group) in &multi_tenant.groups {
            println!("     - {} -> {} ({})", name, group.provider, group.model);
        }
    }
    println!();
    println!("   Ctrl+C to stop");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

/// Create a provider instance based on configuration.
fn create_provider(
    provider_type: &str,
    api_key: &str,
    api_url: Option<&str>,
    model: &str,
) -> anyhow::Result<Arc<dyn Provider>> {
    match provider_type {
        "openrouter" => {
            Ok(Arc::new(crate::providers::openrouter::OpenRouterProvider::new(
                Some(api_key),
            )))
        }
        "openai" => {
            Ok(Arc::new(crate::providers::openai::OpenAiProvider::with_base_url(
                api_url,
                Some(api_key),
            )))
        }
        "anthropic" => {
            Ok(Arc::new(crate::providers::anthropic::AnthropicProvider::new(
                Some(api_key),
            )))
        }
        "glm" => {
            Ok(Arc::new(crate::providers::glm::GlmProvider::new(
                Some(api_key),
            )))
        }
        "ollama" => {
            Ok(Arc::new(crate::providers::ollama::OllamaProvider::new_with_reasoning(
                api_url,
                Some(api_key),
                Some(false),
            )))
        }
        _ => {
            if let Some(base_url) = api_url {
                Ok(Arc::new(
                    crate::providers::compatible::OpenAiCompatibleProvider::new(
                        model,
                        base_url,
                        Some(api_key),
                        crate::providers::compatible::AuthStyle::Bearer,
                    ),
                ))
            } else {
                anyhow::bail!("Unknown provider type: {}", provider_type)
            }
        }
    }
}

/// Health check endpoint.
async fn health_check() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "ok",
        "service": "mclaw-gateway",
        "version": env!("CARGO_PKG_VERSION")
    }))
}

/// Chat request from client.
#[derive(Debug, Deserialize)]
pub struct ChatRequestPayload {
    pub messages: Vec<ChatMessage>,
    #[serde(default)]
    pub temperature: Option<f64>,
    #[serde(default)]
    pub max_tokens: Option<u32>,
}

/// Chat response to client.
#[derive(Debug, Serialize)]
pub struct ChatResponsePayload {
    pub content: String,
    pub model: String,
    #[serde(default)]
    pub usage: Option<TokenUsagePayload>,
}

#[derive(Debug, Serialize)]
pub struct TokenUsagePayload {
    #[serde(default)]
    pub input_tokens: Option<u64>,
    #[serde(default)]
    pub output_tokens: Option<u64>,
}

/// Error response.
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
    #[serde(default)]
    pub detail: Option<String>,
}

/// Handle chat requests.
async fn handle_chat(
    State(state): State<GatewayState>,
    headers: HeaderMap,
    Json(payload): Json<ChatRequestPayload>,
) -> impl IntoResponse {
    // Extract credentials
    let x_client_id = headers.get("x-client-id").and_then(|h| h.to_str().ok());
    let (client_id, _secret) = match extract_client_credentials(
        headers.get("authorization").and_then(|h| h.to_str().ok()),
        x_client_id,
    ) {
        Some(creds) => creds,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(ErrorResponse {
                    error: "Unauthorized".to_string(),
                    detail: Some("Missing or invalid Authorization header".to_string()),
                }),
            )
                .into_response();
        }
    };

    // Get the client group configuration
    let multi_tenant = match &state.config.multi_tenant {
        Some(mt) => mt,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "Multi-tenant not enabled".to_string(),
                    detail: None,
                }),
            )
                .into_response();
        }
    };

    let group = match multi_tenant.groups.get(&client_id) {
        Some(g) => g,
        None => {
            warn!("Unknown client: {}", client_id);
            return (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "Client not found".to_string(),
                    detail: Some(format!("Client '{}' is not configured", client_id)),
                }),
            )
                .into_response();
        }
    };

    info!(
        "Chat request from client={}, provider={}, model={}",
        client_id, group.provider, group.model
    );

    // Get or create the provider for this group
    let provider_key = format!("{}:{}", group.provider, group.api_key.chars().take(4).collect::<String>());
    let provider = match state.providers.get(&provider_key) {
        Some(p) => p.clone(),
        None => {
            error!("Provider not found for client {}: {}", client_id, group.provider);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Provider not available".to_string(),
                    detail: Some(format!("Provider '{}' not configured", group.provider)),
                }),
            )
                .into_response();
        }
    };

    // Build chat request - use simple chat for now
    let temperature = payload.temperature.or(group.temperature).unwrap_or(0.7);

    // Call the provider
    match provider.chat_with_history(&payload.messages, &group.model, temperature).await {
        Ok(text) => {
            info!("Chat success for client={}", client_id);

            (
                StatusCode::OK,
                Json(ChatResponsePayload {
                    content: text,
                    model: group.model.clone(),
                    usage: None, // Usage not tracked in simple chat
                }),
            )
                .into_response()
        }
        Err(e) => {
            error!("Provider error for client {}: {}", client_id, e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Chat failed".to_string(),
                    detail: Some(format!("{}", e)),
                }),
            )
                .into_response()
        }
    }
}

/// List all configured clients.
async fn list_clients(State(state): State<GatewayState>) -> impl IntoResponse {
    let clients: Vec<serde_json::Value> = state
        .config
        .multi_tenant
        .as_ref()
        .map(|mt| {
            mt.groups
                .iter()
                .map(|(name, group)| {
                    serde_json::json!({
                        "client_id": name,
                        "provider": group.provider,
                        "model": group.model,
                        "rate_limit": group.rate_limit,
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    (StatusCode::OK, Json(clients)).into_response()
}

/// Extract client credentials from Authorization header.
fn extract_client_credentials(
    auth_header: Option<&str>,
    x_client_id: Option<&str>,
) -> Option<(String, String)> {
    let auth = auth_header?;

    // Support Bearer token format
    if let Some(token) = auth.strip_prefix("Bearer ") {
        // Format: mc_<client_id>_<hash>
        if let Some(rest) = token.strip_prefix("mc_") {
            if let Some((client_id, _hash)) = rest.rsplit_once('_') {
                return Some((client_id.to_string(), token.to_string()));
            }
        }
        // Try X-Client-Id header
        if let Some(id) = x_client_id {
            return Some((id.to_string(), token.to_string()));
        }
    }

    // Support Basic auth format
    if let Some(credentials) = auth.strip_prefix("Basic ") {
        let mut buffer = Vec::new();
        if base64::engine::general_purpose::STANDARD
            .decode_vec(credentials.as_bytes(), &mut buffer)
            .is_ok()
        {
            let decoded_str = String::from_utf8_lossy(&buffer);
            if let Some((client_id, secret)) = decoded_str.split_once(':') {
                return Some((client_id.to_string(), secret.to_string()));
            }
        }
    }

    None
}

/// Graceful shutdown signal.
async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>;

    tokio::select! {
        _ = ctrl_c => {
            info!("Received Ctrl+C, shutting down...");
        },
        _ = terminate => {
            info!("Received terminate signal, shutting down...");
        },
    }
}

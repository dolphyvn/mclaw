//! Dispatcher service configuration.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Dispatcher service configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceConfig {
    pub server: ServerConfig,
    pub telegram: TelegramConfig,
    pub machines: MachinesConfig,
    pub logging: LoggingConfig,
}

impl Default for ServiceConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            telegram: TelegramConfig::default(),
            machines: MachinesConfig::default(),
            logging: LoggingConfig::default(),
        }
    }
}

/// Server configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Host to bind to.
    #[serde(default = "default_host")]
    pub host: String,

    /// Port to listen on.
    #[serde(default = "default_port")]
    pub port: u16,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_port(),
        }
    }
}

fn default_host() -> String {
    "0.0.0.0".to_string()
}

fn default_port() -> u16 {
    42619
}

/// Telegram configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramConfig {
    /// Bot token.
    #[serde(default = "default_empty_string")]
    pub bot_token: String,

    /// Webhook URL (optional).
    #[serde(default)]
    pub webhook_url: Option<String>,

    /// Allowed users (empty = all users).
    #[serde(default)]
    pub allowed_users: Vec<String>,
}

impl Default for TelegramConfig {
    fn default() -> Self {
        Self {
            bot_token: String::new(),
            webhook_url: None,
            allowed_users: Vec::new(),
        }
    }
}

fn default_empty_string() -> String {
    String::new()
}

/// Machines registry configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MachinesConfig {
    /// Path to machines registry file.
    #[serde(default = "default_machines_path")]
    pub path: PathBuf,
}

impl Default for MachinesConfig {
    fn default() -> Self {
        Self {
            path: default_machines_path(),
        }
    }
}

fn default_machines_path() -> PathBuf {
    PathBuf::from("/etc/mclaw/machines.toml")
}

/// Logging configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// Log level.
    #[serde(default = "default_log_level")]
    pub level: String,

    /// Log file (optional).
    #[serde(default)]
    pub file: Option<String>,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: default_log_level(),
            file: None,
        }
    }
}

fn default_log_level() -> String {
    "info".to_string()
}

/// Machine configuration from registry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MachineConfig {
    /// Machine name (used in @machine syntax).
    pub name: String,

    /// MClaw gateway URL.
    pub url: String,

    /// Optional pairing token for authentication.
    #[serde(default)]
    pub token: Option<String>,

    /// Is this the default machine?
    #[serde(default)]
    pub default: bool,

    /// Optional description.
    #[serde(default)]
    pub description: Option<String>,
}

/// Machines registry file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MachinesRegistry {
    pub machines: Vec<MachineConfig>,
}

impl Default for MachinesRegistry {
    fn default() -> Self {
        Self {
            machines: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = ServiceConfig::default();
        assert_eq!(config.server.host, "0.0.0.0");
        assert_eq!(config.server.port, 42619);
        assert_eq!(config.logging.level, "info");
    }

    #[test]
    fn test_parse_machines_registry() {
        let toml = r#"
            [[machines]]
            name = "client1"
            url = "http://localhost:42618"
            default = true

            [[machines]]
            name = "client2"
            url = "http://51.255.93.22:42618"
        "#;

        let registry: MachinesRegistry = toml::from_str(toml).unwrap();
        assert_eq!(registry.machines.len(), 2);
        assert_eq!(registry.machines[0].name, "client1");
        assert!(registry.machines[0].default);
        assert_eq!(registry.machines[1].name, "client2");
        assert!(!registry.machines[1].default);
    }
}

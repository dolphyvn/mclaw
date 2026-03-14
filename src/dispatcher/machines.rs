//! Machine registry management with dynamic registration support.

use super::config::{MachineConfig, MachinesRegistry};
use anyhow::{Context, Result};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Machine registration request.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RegistrationRequest {
    pub machine_name: String,
    pub url: String,
    pub auth_token: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub default: bool,
}

/// Machine heartbeat info.
#[derive(Debug, Clone)]
struct MachineHeartbeat {
    machine_name: String,
    last_seen: Instant,
}

/// Shared machine registry with dynamic registration support.
#[derive(Clone)]
pub struct MachineRegistry {
    inner: Arc<RwLock<RegistryInner>>,
}

#[derive(Clone)]
struct RegistryInner {
    /// All machines (static from config + dynamic registrations)
    machines: HashMap<String, MachineConfig>,
    /// Default machine name
    default_machine: Option<String>,
    /// Heartbeat tracking for dynamic machines
    heartbeats: HashMap<String, Instant>,
    /// Authentication tokens for registration
    auth_tokens: HashMap<String, String>,
    /// Heartbeat timeout (default 60 seconds)
    heartbeat_timeout: Duration,
}

impl MachineRegistry {
    /// Load registry from file.
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();

        let registry: MachinesRegistry = if path.exists() {
            let content = fs::read_to_string(path)
                .with_context(|| format!("Failed to read machines file: {}", path.display()))?;
            toml::from_str(&content)
                .with_context(|| format!("Failed to parse machines file: {}", path.display()))?
        } else {
            tracing::warn!("Machines file not found: {}, using empty registry", path.display());
            MachinesRegistry::default()
        };

        let mut machines = HashMap::new();
        let mut default_machine = None;

        for machine in registry.machines {
            let name = machine.name.clone();
            if machine.default {
                default_machine = Some(name.clone());
            }
            machines.insert(name, machine);
        }

        Ok(Self {
            inner: Arc::new(RwLock::new(RegistryInner {
                machines,
                default_machine,
                heartbeats: HashMap::new(),
                auth_tokens: HashMap::new(),
                heartbeat_timeout: Duration::from_secs(60),
            })),
        })
    }

    /// Create a new empty registry (for testing).
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(RegistryInner {
                machines: HashMap::new(),
                default_machine: None,
                heartbeats: HashMap::new(),
                auth_tokens: HashMap::new(),
                heartbeat_timeout: Duration::from_secs(60),
            })),
        }
    }

    /// Create with custom heartbeat timeout.
    pub fn with_timeout(timeout: Duration) -> Self {
        let registry = Self::new();
        registry.inner.write().heartbeat_timeout = timeout;
        registry
    }

    /// Get machine by name.
    pub fn get(&self, name: &str) -> Option<MachineConfig> {
        self.inner.read().machines.get(name).cloned()
    }

    /// Get default machine.
    pub fn get_default(&self) -> Option<MachineConfig> {
        let inner = self.inner.read();
        inner
            .default_machine
            .as_ref()
            .and_then(|name| inner.machines.get(name))
            .cloned()
    }

    /// List all machine names.
    pub fn list_names(&self) -> Vec<String> {
        self.inner
            .read()
            .machines
            .keys()
            .cloned()
            .collect()
    }

    /// List all machines.
    pub fn list_all(&self) -> Vec<MachineConfig> {
        self.inner
            .read()
            .machines
            .values()
            .cloned()
            .collect()
    }

    /// Check if machine exists.
    pub fn contains(&self, name: &str) -> bool {
        self.inner.read().machines.contains_key(name)
    }

    /// Reload registry from file.
    pub fn reload<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let new_registry = Self::load(path)?;
        let mut inner = self.inner.write();
        // Preserve heartbeats and auth tokens
        let heartbeats = std::mem::take(&mut inner.heartbeats);
        let auth_tokens = std::mem::take(&mut inner.auth_tokens);
        let heartbeat_timeout = inner.heartbeat_timeout;
        *inner = new_registry.inner.read().clone();
        inner.heartbeats = heartbeats;
        inner.auth_tokens = auth_tokens;
        inner.heartbeat_timeout = heartbeat_timeout;
        Ok(())
    }

    /// Register a new machine dynamically.
    pub fn register(&self, req: RegistrationRequest, expected_token: Option<&str>) -> Result<()> {
        let mut inner = self.inner.write();

        // Check auth token if provided
        if let Some(expected) = expected_token {
            if let Some(provided) = &req.auth_token {
                if provided != expected {
                    anyhow::bail!("Invalid auth token");
                }
            } else {
                anyhow::bail!("Auth token required");
            }
        }

        // Don't allow overwriting existing machines with same name
        if inner.machines.contains_key(&req.machine_name) {
            tracing::info!("Machine {} already registered, updating heartbeat", req.machine_name);
            inner.heartbeats.insert(req.machine_name.clone(), Instant::now());
            return Ok(());
        }

        let machine_name = req.machine_name.clone();
        let machine_url = req.url.clone();

        let machine = MachineConfig {
            name: machine_name.clone(),
            url: machine_url.clone(),
            token: req.auth_token.clone(),
            default: req.default,
            description: req.description.clone(),
        };

        if machine.default {
            inner.default_machine = Some(machine.name.clone());
        }

        inner.machines.insert(machine.name.clone(), machine);
        inner.heartbeats.insert(machine_name.clone(), Instant::now());

        if let Some(token) = req.auth_token {
            inner.auth_tokens.insert(machine_name.clone(), token);
        }

        tracing::info!("Registered machine: {} at {}", machine_name, machine_url);
        Ok(())
    }

    /// Unregister a machine.
    pub fn unregister(&self, machine_name: &str, expected_token: Option<&str>) -> Result<()> {
        let mut inner = self.inner.write();

        // Check auth token if the machine has one
        if let Some(expected) = expected_token {
            if let Some(stored) = inner.auth_tokens.get(machine_name) {
                if stored != expected {
                    anyhow::bail!("Invalid auth token");
                }
            }
        }

        if inner.machines.remove(machine_name).is_some() {
            inner.heartbeats.remove(machine_name);
            inner.auth_tokens.remove(machine_name);

            // Update default if this was the default
            if inner.default_machine.as_deref() == Some(machine_name) {
                inner.default_machine = None;
            }

            tracing::info!("Unregistered machine: {}", machine_name);
            Ok(())
        } else {
            anyhow::bail!("Machine not found: {}", machine_name);
        }
    }

    /// Update heartbeat for a machine.
    pub fn heartbeat(&self, machine_name: &str) -> Result<bool> {
        let mut inner = self.inner.write();

        if !inner.machines.contains_key(machine_name) {
            return Ok(false);
        }

        inner.heartbeats.insert(machine_name.to_string(), Instant::now());
        Ok(true)
    }

    /// Remove stale machines (no heartbeat within timeout).
    pub fn remove_stale(&self) -> Vec<String> {
        let mut inner = self.inner.write();
        let timeout = inner.heartbeat_timeout;
        let now = Instant::now();
        let mut stale = Vec::new();

        // First pass: collect stale machine names
        inner.heartbeats.retain(|name, last_seen| {
            let is_alive = now.duration_since(*last_seen) < timeout;
            if !is_alive {
                stale.push(name.clone());
            }
            is_alive
        });

        // Second pass: remove stale machines
        for name in &stale {
            tracing::warn!("Machine {} is stale, removing", name);
            inner.machines.remove(name);
            inner.auth_tokens.remove(name);
            inner.heartbeats.remove(name);
        }

        // Update default if it was removed
        if let Some(default_name) = &inner.default_machine {
            if !inner.machines.contains_key(default_name) {
                inner.default_machine = None;
            }
        }

        stale
    }

    /// Check machine health status.
    pub fn get_health_status(&self) -> Vec<(String, bool)> {
        let inner = self.inner.read();
        let now = Instant::now();
        let timeout = inner.heartbeat_timeout;

        inner.machines.keys()
            .map(|name| {
                let is_healthy = inner.heartbeats.get(name)
                    .map(|last| now.duration_since(*last) < timeout)
                    .unwrap_or(true); // Static machines are always healthy
                (name.clone(), is_healthy)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_registry() {
        let registry = MachineRegistry::load("/nonexistent/path.toml").unwrap();
        assert_eq!(registry.list_names().len(), 0);
        assert!(registry.get_default().is_none());
    }

    #[test]
    fn test_machine_lookup() {
        use std::time::Duration;

        let mut inner = RegistryInner {
            machines: HashMap::new(),
            default_machine: None,
            heartbeats: HashMap::new(),
            auth_tokens: HashMap::new(),
            heartbeat_timeout: Duration::from_secs(60),
        };

        let m1 = MachineConfig {
            name: "client1".to_string(),
            url: "http://localhost:42618".to_string(),
            token: None,
            default: true,
            description: None,
        };

        inner.machines.insert("client1".to_string(), m1.clone());
        inner.default_machine = Some("client1".to_string());

        let registry = MachineRegistry {
            inner: Arc::new(RwLock::new(inner)),
        };

        assert!(registry.contains("client1"));
        assert!(!registry.contains("client2"));

        let found = registry.get("client1").unwrap();
        assert_eq!(found.name, "client1");
        assert_eq!(found.url, "http://localhost:42618");

        let default = registry.get_default().unwrap();
        assert_eq!(default.name, "client1");
    }
}

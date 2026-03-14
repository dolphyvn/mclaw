//! Authentication for multi-tenant gateway.

use base64::Engine;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Generate a new client secret for a group.
pub fn generate_client_secret(client_id: &str) -> String {
    let bytes: [u8; 32] = rand::random();
    format!("mc_{}_{}", client_id, hex::encode(bytes))
}

/// Hash a client secret for storage.
pub fn hash_client_secret(secret: &str) -> String {
    format!("{:x}", Sha256::digest(secret.as_bytes()))
}

/// Verify a client secret against a stored hash.
pub fn verify_client_secret(secret: &str, hash: &str) -> bool {
    hash_client_secret(secret) == hash
}

/// Authentication manager for client groups.
#[derive(Clone)]
pub struct AuthManager {
    /// Client secrets (hashed)
    secrets: Arc<RwLock<HashMap<String, String>>>,
}

impl AuthManager {
    /// Create a new auth manager.
    pub fn new() -> Self {
        Self {
            secrets: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Add a client secret (hashed).
    pub async fn add_client(&self, client_id: String, hashed_secret: String) {
        let mut secrets = self.secrets.write().await;
        secrets.insert(client_id, hashed_secret);
    }

    /// Verify a client's credentials.
    pub async fn verify_client(&self, client_id: &str, secret: &str) -> bool {
        let secrets = self.secrets.read().await;
        if let Some(stored_hash) = secrets.get(client_id) {
            verify_client_secret(secret, stored_hash)
        } else {
            false
        }
    }

    /// Load clients from configuration.
    pub async fn load_from_config(&self, groups: &HashMap<String, crate::multi_tenant::config::ClientGroup>) {
        let mut secrets = self.secrets.write().await;
        secrets.clear();
        for (name, group) in groups {
            secrets.insert(name.clone(), group.client_secret.clone());
        }
    }
}

impl Default for AuthManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Extract client credentials from Authorization header.
pub fn extract_client_credentials(auth_header: Option<&str>, x_client_id: Option<&str>) -> Option<(String, String)> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_client_secret() {
        let secret = generate_client_secret("test-group");
        assert!(secret.starts_with("mc_test-group_"));
        assert!(secret.len() > "mc_test-group_".len() + 20);
    }

    #[test]
    fn test_hash_client_secret() {
        let secret = "test_secret";
        let hash1 = hash_client_secret(secret);
        let hash2 = hash_client_secret(secret);
        assert_eq!(hash1, hash2);
        assert_eq!(hash1.len(), 64);
    }

    #[test]
    fn test_verify_client_secret() {
        let secret = "test_secret";
        let hash = hash_client_secret(secret);
        assert!(verify_client_secret(secret, &hash));
        assert!(!verify_client_secret("wrong", &hash));
    }
}

//! Telegram webhook handler for dispatcher.

use super::router::CommandRouter;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::json;

/// Telegram webhook update.
#[derive(Debug, Deserialize)]
pub struct TelegramUpdate {
    pub update_id: i64,
    #[serde(default)]
    pub message: Option<TelegramMessage>,
}

/// Telegram message.
#[derive(Debug, Deserialize, Clone)]
pub struct TelegramMessage {
    pub message_id: i64,
    pub chat: TelegramChat,
    pub from: Option<TelegramUser>,
    pub text: Option<String>,
}

/// Telegram chat.
#[derive(Debug, Deserialize, Clone)]
pub struct TelegramChat {
    pub id: i64,
    #[serde(rename = "type")]
    pub chat_type: Option<String>,
    pub title: Option<String>,
}

/// Telegram user.
#[derive(Debug, Deserialize, Clone)]
pub struct TelegramUser {
    pub id: i64,
    pub username: Option<String>,
    pub first_name: Option<String>,
}

/// Outgoing Telegram message.
#[derive(Debug, Serialize)]
struct TelegramOutgoingMessage {
    pub chat_id: i64,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reply_to_message_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parse_mode: Option<String>,
}

/// Telegram webhook handler.
pub struct TelegramHandler {
    bot_token: String,
    bot_username: String,
    router: CommandRouter,
    allowed_users: Vec<String>,
}

impl TelegramHandler {
    /// Create a new handler.
    pub fn new(
        bot_token: String,
        bot_username: String,
        router: CommandRouter,
        allowed_users: Vec<String>,
    ) -> Self {
        Self {
            bot_token,
            bot_username,
            router,
            allowed_users,
        }
    }

    /// Strip bot mention from message text (for group chats).
    /// Converts "@botname @client1 command" -> "@client1 command"
    fn strip_bot_mention(&self, text: &str) -> String {
        let text = text.trim();

        // If no bot username set, return as-is
        if self.bot_username.is_empty() {
            return text.to_string();
        }

        // Try various formats:
        // @botname command
        // @botname@client1 command (no space)
        let bot_mention = format!("@{}", self.bot_username);

        // Check if text starts with bot mention
        if text.starts_with(&bot_mention) {
            let rest = text[bot_mention.len()..].trim();
            return rest.to_string();
        }

        // Also handle case where bot name is directly adjacent: @botname@client1
        if let Some(pos) = text.find(&bot_mention) {
            if pos == 0 {
                let rest = &text[bot_mention.len()..];
                // If next char is @, it's @botname@client1 format
                if rest.starts_with('@') {
                    return rest.to_string();
                }
                // Otherwise it's @botname command format, trim and return
                return rest.trim().to_string();
            }
        }

        text.to_string()
    }

    /// Check if user is allowed.
    fn is_user_allowed(&self, user: &TelegramUser) -> bool {
        if self.allowed_users.is_empty() || self.allowed_users.contains(&"*".to_string()) {
            return true;
        }

        if let Some(username) = &user.username {
            let username_with_at = format!("@{}", username);
            if self.allowed_users.contains(&username_with_at)
                || self.allowed_users.contains(username)
            {
                return true;
            }
        }

        false
    }

    /// Format user identity for logging.
    fn format_user(&self, user: &TelegramUser) -> String {
        if let Some(username) = &user.username {
            format!("@{}", username)
        } else if let Some(first_name) = &user.first_name {
            first_name.clone()
        } else {
            format!("user_{}", user.id)
        }
    }

    /// Handle an incoming update.
    pub async fn handle_update(&self, update: TelegramUpdate) -> Result<()> {
        let message = match update.message {
            Some(m) => m,
            None => return Ok(()),
        };

        // Check user permissions
        if let Some(from) = &message.from {
            if !self.is_user_allowed(from) {
                tracing::warn!(
                    "Unauthorized access attempt by {}",
                    self.format_user(from)
                );
                self.send_message(
                    message.chat.id,
                    "Sorry, you are not authorized to use this bot.",
                    Some(message.message_id),
                )
                .await?;
                return Ok(());
            }
        }

        let text = match &message.text {
            Some(t) => t,
            None => return Ok(()), // Ignore non-text messages
        };

        // Strip bot mention from group chat messages
        // Converts "@botname @client1 command" -> "@client1 command"
        let text = self.strip_bot_mention(text);

        let user_display = message
            .from
            .as_ref()
            .map(|u| self.format_user(u))
            .unwrap_or_else(|| "unknown".to_string());

        tracing::info!("Received from {}: {}", user_display, text);

        // Parse and execute command
        let parsed = match self.router.parse(&text) {
            Ok(p) => p,
            Err(e) => {
                let msg = format!("Failed to parse command: {}", e);
                self.send_message(
                    message.chat.id,
                    &msg,
                    Some(message.message_id),
                )
                .await?;
                return Ok(());
            }
        };

        let response = match self.router.execute(&parsed).await {
            Ok(r) => r,
            Err(e) => {
                let msg = format!("Error: {}", e);
                self.send_message(
                    message.chat.id,
                    &msg,
                    Some(message.message_id),
                )
                .await?;
                return Ok(());
            }
        };

        // Send response
        let formatted = response.format_for_telegram();
        self.send_chunked_message(message.chat.id, &formatted, Some(message.message_id))
            .await?;

        Ok(())
    }

    /// Send a message to Telegram.
    async fn send_message(
        &self,
        chat_id: i64,
        text: &str,
        reply_to: Option<i64>,
    ) -> Result<()> {
        let url = format!(
            "https://api.telegram.org/bot{}/sendMessage",
            self.bot_token
        );

        let payload = TelegramOutgoingMessage {
            chat_id,
            text: text.to_string(),
            reply_to_message_id: reply_to,
            parse_mode: Some("Markdown".to_string()),
        };

        let client = reqwest::Client::new();
        let resp = client
            .post(&url)
            .json(&payload)
            .send()
            .await?
            .text()
            .await?;

        tracing::debug!("Telegram response: {}", resp);
        Ok(())
    }

    /// Send a message in chunks if too long.
    async fn send_chunked_message(
        &self,
        chat_id: i64,
        text: &str,
        reply_to: Option<i64>,
    ) -> Result<()> {
        const MAX_SIZE: usize = 4000; // Telegram limit is 4096

        if text.len() <= MAX_SIZE {
            return self.send_message(chat_id, text, reply_to).await;
        }

        // Split into chunks
        let chunks: Vec<&str> = text
            .as_bytes()
            .chunks(MAX_SIZE)
            .map(|b| std::str::from_utf8(b).unwrap_or(""))
            .collect();

        for (i, chunk) in chunks.iter().enumerate() {
            let prefix = if chunks.len() > 1 {
                format!("({}/{})\n", i + 1, chunks.len())
            } else {
                String::new()
            };
            self.send_message(chat_id, &format!("{}{}", prefix, chunk), reply_to)
                .await?;
        }

        Ok(())
    }

    /// Set the webhook URL.
    pub async fn set_webhook(&self, webhook_url: &str) -> Result<()> {
        let url = format!(
            "https://api.telegram.org/bot{}/setWebhook",
            self.bot_token
        );

        let payload = json!({ "url": webhook_url });

        let client = reqwest::Client::new();
        let resp = client.post(&url).json(&payload).send().await?;

        if resp.status().is_success() {
            tracing::info!("Webhook set to: {}", webhook_url);
            Ok(())
        } else {
            anyhow::bail!("Failed to set webhook: {}", resp.status())
        }
    }

    /// Get webhook info.
    pub async fn get_webhook_info(&self) -> Result<serde_json::Value> {
        let url = format!(
            "https://api.telegram.org/bot{}/getWebhookInfo",
            self.bot_token
        );

        let client = reqwest::Client::new();
        let resp = client.get(&url).send().await?.json().await?;
        Ok(resp)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_allowed_wildcard() {
        let registry = MachineRegistry::load("/nonexistent").unwrap();
        let router = CommandRouter::new(registry);
        let handler = TelegramHandler::new("test_token".to_string(), "testbot".to_string(), router, vec!["*".to_string()]);

        let user = TelegramUser {
            id: 123,
            username: Some("testuser".to_string()),
            first_name: Some("Test".to_string()),
        };

        assert!(handler.is_user_allowed(&user));
    }

    #[test]
    fn test_user_allowed_specific() {
        let registry = MachineRegistry::load("/nonexistent").unwrap();
        let router = CommandRouter::new(registry);
        let handler = TelegramHandler::new(
            "test_token".to_string(),
            "testbot".to_string(),
            router,
            vec!["@allowed".to_string()],
        );

        let user = TelegramUser {
            id: 123,
            username: Some("allowed".to_string()),
            first_name: Some("Test".to_string()),
        };

        assert!(handler.is_user_allowed(&user));
    }

    #[test]
    fn test_user_denied() {
        let registry = MachineRegistry::load("/nonexistent").unwrap();
        let router = CommandRouter::new(registry);
        let handler = TelegramHandler::new(
            "test_token".to_string(),
            "testbot".to_string(),
            router,
            vec!["@allowed".to_string()],
        );

        let user = TelegramUser {
            id: 123,
            username: Some("blocked".to_string()),
            first_name: Some("Test".to_string()),
        };

        assert!(!handler.is_user_allowed(&user));
    }
}

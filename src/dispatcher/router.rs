//! Command routing logic.

use super::client::MClawClient;
use super::machines::MachineRegistry;
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

/// Parsed command with target.
#[derive(Debug, Clone)]
pub struct ParsedCommand {
    pub target: CommandTarget,
    pub command: String,
}

/// Command target.
#[derive(Debug, Clone, PartialEq)]
pub enum CommandTarget {
    /// All machines.
    All,
    /// Specific machine by name.
    Machine(String),
    /// Default machine.
    Default,
    /// List machines command.
    List,
}

/// Response from a machine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MachineResponse {
    pub machine: String,
    pub response: String,
    pub error: Option<String>,
}

/// Aggregated response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DispatcherResponse {
    pub responses: Vec<MachineResponse>,
}

impl DispatcherResponse {
    /// Single machine response.
    pub fn single(machine: String, response: String) -> Self {
        Self {
            responses: vec![MachineResponse {
                machine,
                response,
                error: None,
            }],
        }
    }

    /// Error response.
    pub fn error(machine: String, error: String) -> Self {
        Self {
            responses: vec![MachineResponse {
                machine,
                response: String::new(),
                error: Some(error),
            }],
        }
    }

    /// Format for Telegram.
    pub fn format_for_telegram(&self) -> String {
        if self.responses.len() == 1 {
            let resp = &self.responses[0];
            if let Some(err) = &resp.error {
                return format!("❌ @{}: {}", resp.machine, err);
            }
            return format!("@{}: {}", resp.machine, resp.response);
        }

        let mut output = String::new();
        for resp in &self.responses {
            output.push_str(&format!("**@{}**\n", resp.machine));
            if let Some(err) = &resp.error {
                output.push_str(&format!("❌ Error: {}\n", err));
            } else {
                // Truncate very long responses
                let content = if resp.response.len() > 1000 {
                    format!("{}...\n(truncated)", &resp.response[..1000])
                } else {
                    resp.response.clone()
                };
                output.push_str(&content);
                output.push_str("\n\n");
            }
        }
        output
    }
}

/// Command router.
#[derive(Clone)]
pub struct CommandRouter {
    pub registry: MachineRegistry,
}

impl CommandRouter {
    /// Create a new router.
    pub fn new(registry: MachineRegistry) -> Self {
        Self { registry }
    }

    /// Parse a command message.
    pub fn parse(&self, message: &str) -> Result<ParsedCommand> {
        let trimmed = message.trim();

        // Check for @list
        if trimmed == "@list" {
            return Ok(ParsedCommand {
                target: CommandTarget::List,
                command: String::new(),
            });
        }

        // Check for @all
        if let Some(rest) = trimmed.strip_prefix("@all ") {
            return Ok(ParsedCommand {
                target: CommandTarget::All,
                command: rest.to_string(),
            });
        }
        if trimmed == "@all" {
            return Ok(ParsedCommand {
                target: CommandTarget::All,
                command: String::new(),
            });
        }

        // Check for @machine_name
        if let Some(rest) = trimmed.strip_prefix('@') {
            if let Some((machine_name, command)) = rest.split_once(' ') {
                let machine_name = machine_name.to_string();
                if self.registry.contains(&machine_name) {
                    return Ok(ParsedCommand {
                        target: CommandTarget::Machine(machine_name),
                        command: command.to_string(),
                    });
                } else if machine_name == "all" {
                    return Ok(ParsedCommand {
                        target: CommandTarget::All,
                        command: command.to_string(),
                    });
                }
            } else if self.registry.contains(rest) || rest == "all" {
                let target = if rest == "all" {
                    CommandTarget::All
                } else {
                    CommandTarget::Machine(rest.to_string())
                };
                return Ok(ParsedCommand {
                    target,
                    command: String::new(),
                });
            }
        }

        // No prefix - use default machine
        Ok(ParsedCommand {
            target: CommandTarget::Default,
            command: trimmed.to_string(),
        })
    }

    /// Execute a command.
    pub async fn execute(&self, parsed: &ParsedCommand) -> Result<DispatcherResponse> {
        match &parsed.target {
            CommandTarget::List => {
                let machines = self.registry.list_all();
                let response = if machines.is_empty() {
                    "No machines configured.".to_string()
                } else {
                    let mut output = String::from("Configured machines:\n");
                    for m in machines {
                        let default_marker = if m.default { " (default)" } else { "" };
                        output.push_str(&format!(
                            "  @{}{} - {}\n",
                            m.name, default_marker, m.url
                        ));
                    }
                    output
                };
                Ok(DispatcherResponse::single("dispatcher".to_string(), response))
            }
            CommandTarget::All => {
                let machines = self.registry.list_all();
                if machines.is_empty() {
                    bail!("No machines configured");
                }

                let mut responses = Vec::new();
                for machine in machines {
                    let client = MClawClient::from_config(&machine);
                    match client.send_command(&parsed.command).await {
                        Ok(resp) => {
                            responses.push(MachineResponse {
                                machine: machine.name.clone(),
                                response: resp,
                                error: None,
                            });
                        }
                        Err(e) => {
                            responses.push(MachineResponse {
                                machine: machine.name.clone(),
                                response: String::new(),
                                error: Some(e.to_string()),
                            });
                        }
                    }
                }
                Ok(DispatcherResponse { responses })
            }
            CommandTarget::Machine(name) => {
                let machine = self
                    .registry
                    .get(name)
                    .ok_or_else(|| anyhow::anyhow!("Machine not found: {}", name))?;
                let client = MClawClient::from_config(&machine);
                match client.send_command(&parsed.command).await {
                    Ok(resp) => Ok(DispatcherResponse::single(name.clone(), resp)),
                    Err(e) => Ok(DispatcherResponse::error(name.clone(), e.to_string())),
                }
            }
            CommandTarget::Default => {
                let machine = self.registry.get_default().ok_or_else(|| {
                    anyhow::anyhow!("No default machine configured")
                })?;
                let client = MClawClient::from_config(&machine);
                match client.send_command(&parsed.command).await {
                    Ok(resp) => Ok(DispatcherResponse::single(machine.name.clone(), resp)),
                    Err(e) => Ok(DispatcherResponse::error(machine.name.clone(), e.to_string())),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dispatcher::machines::MachineRegistry;

    #[test]
    fn test_parse_list_command() {
        let registry = MachineRegistry::load("/nonexistent").unwrap();
        let router = CommandRouter::new(registry);

        let parsed = router.parse("@list").unwrap();
        assert_eq!(parsed.target, CommandTarget::List);
    }

    #[test]
    fn test_parse_all_command() {
        let registry = MachineRegistry::load("/nonexistent").unwrap();
        let router = CommandRouter::new(registry);

        let parsed = router.parse("@all uptime").unwrap();
        assert_eq!(parsed.target, CommandTarget::All);
        assert_eq!(parsed.command, "uptime");
    }

    #[test]
    fn test_parse_default_command() {
        let registry = MachineRegistry::load("/nonexistent").unwrap();
        let router = CommandRouter::new(registry);

        let parsed = router.parse("uptime").unwrap();
        assert_eq!(parsed.target, CommandTarget::Default);
        assert_eq!(parsed.command, "uptime");
    }
}

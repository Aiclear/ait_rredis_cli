use std::collections::HashMap;
use crate::redis_client::RedisClient;
use crate::redis_type::{RespType, RedisCommandInfo};

#[derive(Debug, Clone)]
pub struct CommandDoc {
    pub name: String,
    pub summary: String,
    pub arity: isize,
    pub arguments: Option<Vec<Argument>>,
    pub flags: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Argument {
    pub name: String,
    pub r#type: Option<String>,
    pub optional: Option<bool>,
    pub multiple: Option<bool>,
    pub description: Option<String>,
}

impl From<&RedisCommandInfo> for CommandDoc {
    fn from(info: &RedisCommandInfo) -> Self {
        let summary = format!("Redis command (arity: {})", info.arity);
        let arguments = if info.arity != 0 {
            Some(vec![Argument {
                name: if info.arity < 0 {
                    "arguments...".to_string()
                } else {
                    "arg".to_string()
                },
                r#type: Some("string".to_string()),
                optional: Some(info.arity < 0),
                multiple: Some(info.arity < 0),
                description: None,
            }])
        } else {
            None
        };

        CommandDoc {
            name: info.name.clone(),
            summary,
            arity: info.arity,
            arguments,
            flags: info.flags.clone(),
        }
    }
}

pub struct CommandDocs {
    commands: HashMap<String, CommandDoc>,
}

impl CommandDocs {
    pub fn new() -> Self {
        CommandDocs {
            commands: HashMap::new(),
        }
    }

    /// Fetch all commands from Redis using COMMAND command
    pub fn fetch_from_redis(client: &mut RedisClient) -> anyhow::Result<Self> {
        let command = RespType::create_from_command_line("COMMAND");
        client.write_command(command)?;
        
        let response = client.read_resp()?;
        let mut commands = HashMap::new();

        if let Some(arr) = response.as_array() {
            for cmd_resp in arr {
                if let Some(cmd_info) = RedisCommandInfo::from_resp(cmd_resp) {
                    let doc = CommandDoc::from(&cmd_info);
                    commands.insert(doc.name.to_lowercase(), doc.clone());
                    commands.insert(doc.name.to_uppercase(), doc);
                }
            }
        }

        Ok(CommandDocs { commands })
    }

    /// Get command documentation by name
    pub fn get(&self, command: &str) -> Option<&CommandDoc> {
        let cmd = command.to_uppercase();
        self.commands.get(&cmd)
    }

    /// Get command hint for display
    pub fn get_command_hint(&self, command: &str) -> String {
        let cmd = command.to_uppercase();
        if let Some(doc) = self.get(&cmd) {
            let mut hint = format!("{}: {}", cmd, doc.summary);
            
            if let Some(args) = &doc.arguments {
                let arg_strs: Vec<String> = args.iter()
                    .map(|arg| {
                        let prefix = if arg.optional.unwrap_or(false) { "[" } else { "<" };
                        let suffix = if arg.optional.unwrap_or(false) { "]" } else { ">" };
                        let multiple = if arg.multiple.unwrap_or(false) { "..." } else { "" };
                        format!("{}{}{}{}", prefix, arg.name, multiple, suffix)
                    })
                    .collect();
                if !arg_strs.is_empty() {
                    hint.push_str(&format!("\nUsage: {} {}", cmd, arg_strs.join(" ")));
                }
            }

            if !doc.flags.is_empty() {
                hint.push_str(&format!("\nFlags: {}", doc.flags.join(", ")));
            }
            
            hint
        } else {
            format!("No documentation found for '{}'", command)
        }
    }

    /// Get all command names for completion
    pub fn get_all_commands(&self) -> Vec<String> {
        self.commands.keys()
            .filter(|k| k.chars().next().is_some_and(|c| c.is_uppercase()))
            .cloned()
            .collect()
    }

    /// Get completions for prefix
    pub fn get_completions(&self, prefix: &str) -> Vec<String> {
        let upper_prefix = prefix.to_uppercase();
        self.commands
            .keys()
            .filter(|cmd| cmd.starts_with(&upper_prefix) && cmd.chars().next().is_some_and(|c| c.is_uppercase()))
            .cloned()
            .collect()
    }
}

impl Default for CommandDocs {
    fn default() -> Self {
        Self::new()
    }
}

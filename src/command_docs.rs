use std::collections::HashMap;
use serde::Deserialize;
use reqwest::Client;

#[derive(Debug, Deserialize, Clone)]
pub struct RedisCommandDoc {
    pub summary: String,
    pub arguments: Option<Vec<CommandArgument>>,
    pub since: String,
    pub group: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct CommandArgument {
    pub name: String,
    pub r#type: String,
    pub optional: Option<bool>,
    pub multiple: Option<bool>,
    pub description: Option<String>,
}

pub struct CommandDocs {
    client: Client,
    cache: HashMap<String, RedisCommandDoc>,
}

impl CommandDocs {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            cache: HashMap::new(),
        }
    }

    pub async fn get_command_doc(&mut self, command: &str) -> Option<RedisCommandDoc> {
        let command_upper = command.to_uppercase();
        
        if let Some(doc) = self.cache.get(&command_upper) {
            return Some(doc.clone());
        }

        let url = format!("https://redis.io/commands/{}.json", command.to_lowercase());
        
        match self.client.get(&url).send().await {
            Ok(response) => {
                if response.status().is_success() {
                    match response.json::<RedisCommandDoc>().await {
                        Ok(doc) => {
                            self.cache.insert(command_upper.clone(), doc.clone());
                            Some(doc)
                        }
                        Err(_) => None,
                    }
                } else {
                    None
                }
            }
            Err(_) => None,
        }
    }

    pub fn get_hint(&self, command: &str) -> String {
        let command_upper = command.to_uppercase();
        
        if let Some(doc) = self.cache.get(&command_upper) {
            let mut hint = format!("{}: {}", command_upper, doc.summary);
            
            if let Some(args) = &doc.arguments {
                let arg_strs: Vec<String> = args.iter()
                    .map(|arg| {
                        let prefix = if arg.optional.unwrap_or(false) { "[" } else { "<" };
                        let suffix = if arg.optional.unwrap_or(false) { "]" } else { ">" };
                        let multiple = if arg.multiple.unwrap_or(false) { "..." } else { "" };
                        format!("{}{}{}{}", prefix, arg.name, multiple, suffix)
                    })
                    .collect();
                hint.push_str(&format!("\nUsage: {} {}", command_upper, arg_strs.join(" ")));
            }
            
            hint
        } else {
            format!("{}: Command not found in documentation", command_upper)
        }
    }
}

use std::collections::HashMap;

use crate::{
    redis_client::RedisClient,
    redis_type::RespType,
};

pub struct CommandHints {
    cache: HashMap<String, CommandDoc>,
}

#[derive(Debug, Clone)]
pub struct CommandDoc {
    pub summary: String,
    pub group: String,
    pub arguments: Vec<ArgDoc>,
}

#[derive(Debug, Clone)]
pub struct ArgDoc {
    pub name: String,
    pub typ: String,
    pub optional: bool,
    pub multiple: bool,
}

impl CommandHints {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
        }
    }

    pub fn get_hint(&mut self, client: &mut RedisClient, command: &str) -> Option<String> {
        let cmd_upper = command.to_uppercase();
        
        if let Some(doc) = self.cache.get(&cmd_upper) {
            return Some(self.format_hint(doc));
        }

        let doc = self.fetch_command_doc(client, &cmd_upper)?;
        let hint = self.format_hint(&doc);
        self.cache.insert(cmd_upper, doc);
        Some(hint)
    }

    fn fetch_command_doc(&self, client: &mut RedisClient, command: &str) -> Option<CommandDoc> {
        let cmd = format!("COMMAND DOCS {}", command);
        let resp_type = RespType::create_from_command_line(&cmd);
        
        client.write_command(resp_type).ok()?;
        let response = client.read_resp().ok()?;

        self.parse_command_docs_response(&response, command)
    }

    fn parse_command_docs_response(&self, response: &RespType, command: &str) -> Option<CommandDoc> {
        match response {
            RespType::Maps(map) => {
                for (key, value) in map.map.iter() {
                    if let RespType::BulkStrings(bs) = &key.1 {
                        if bs.value.to_uppercase() == command.to_uppercase() {
                            return self.parse_doc_detail(value);
                        }
                    }
                }
                None
            }
            _ => None,
        }
    }

    fn parse_doc_detail(&self, value: &RespType) -> Option<CommandDoc> {
        match value {
            RespType::Maps(doc_map) => {
                let mut summary = String::new();
                let mut group = String::new();
                let mut arguments = Vec::new();

                for (key, val) in doc_map.map.iter() {
                    if let RespType::BulkStrings(key_bs) = &key.1 {
                        match key_bs.value.as_str() {
                            "summary" => {
                                if let RespType::BulkStrings(v) = val {
                                    summary = v.value.clone();
                                }
                            }
                            "group" => {
                                if let RespType::BulkStrings(v) = val {
                                    group = v.value.clone();
                                }
                            }
                            "arguments" => {
                                if let RespType::Arrays(arr) = val {
                                    arguments = self.parse_arguments(arr);
                                }
                            }
                            _ => {}
                        }
                    }
                }

                Some(CommandDoc {
                    summary,
                    group,
                    arguments,
                })
            }
            _ => None,
        }
    }

    fn parse_arguments(&self, arr: &crate::redis_type::Array) -> Vec<ArgDoc> {
        let mut args = Vec::new();
        
        for item in &arr.value {
            if let RespType::Maps(arg_map) = item {
                let mut name = String::new();
                let mut typ = String::new();
                let mut optional = false;
                let mut multiple = false;

                for (key, val) in arg_map.map.iter() {
                    if let RespType::BulkStrings(key_bs) = &key.1 {
                        match key_bs.value.as_str() {
                            "name" => {
                                if let RespType::BulkStrings(v) = val {
                                    name = v.value.clone();
                                } else if let RespType::Arrays(v) = val {
                                    name = v.value.iter()
                                        .filter_map(|e| {
                                            if let RespType::BulkStrings(bs) = e {
                                                Some(bs.value.clone())
                                            } else {
                                                None
                                            }
                                        })
                                        .collect::<Vec<_>>()
                                        .join(" ");
                                }
                            }
                            "type" => {
                                if let RespType::BulkStrings(v) = val {
                                    typ = v.value.clone();
                                }
                            }
                            "optional" => {
                                if let RespType::Integers(i) = val {
                                    optional = i.value != 0;
                                }
                            }
                            "multiple" => {
                                if let RespType::Integers(i) = val {
                                    multiple = i.value != 0;
                                }
                            }
                            _ => {}
                        }
                    }
                }

                args.push(ArgDoc {
                    name,
                    typ,
                    optional,
                    multiple,
                });
            }
        }
        
        args
    }

    fn format_hint(&self, doc: &CommandDoc) -> String {
        let mut hint = String::new();
        
        if !doc.summary.is_empty() {
            hint.push_str(&format!("Summary: {}\n", doc.summary));
        }
        
        if !doc.group.is_empty() {
            hint.push_str(&format!("Group: {}\n", doc.group));
        }
        
        if !doc.arguments.is_empty() {
            hint.push_str("Arguments:\n");
            for (i, arg) in doc.arguments.iter().take(5).enumerate() {
                let optional_mark = if arg.optional { " [optional]" } else { "" };
                let multiple_mark = if arg.multiple { " [multiple]" } else { "" };
                hint.push_str(&format!(
                    "  {}. {} ({}){}{}\n",
                    i + 1,
                    arg.name,
                    arg.typ,
                    optional_mark,
                    multiple_mark
                ));
            }
            if doc.arguments.len() > 5 {
                hint.push_str(&format!("  ... and {} more arguments\n", doc.arguments.len() - 5));
            }
        }
        
        hint.trim_end().to_string()
    }

    pub fn get_inline_hint(&mut self, client: &mut RedisClient, command: &str, arg_index: usize) -> Option<String> {
        let cmd = command.to_uppercase();
        
        if let Some(doc) = self.cache.get(&cmd) {
            return self.format_inline_hint(doc, arg_index);
        }

        if let Some(doc) = self.fetch_command_doc(client, &cmd) {
            let hint = self.format_inline_hint(&doc, arg_index);
            self.cache.insert(cmd, doc);
            return hint;
        }
        
        None
    }

    pub fn get_cached_hint(&self, command: &str, arg_index: usize) -> Option<String> {
        let cmd = command.to_uppercase();
        
        if let Some(doc) = self.cache.get(&cmd) {
            return self.format_inline_hint(doc, arg_index);
        }
        
        None
    }

    pub fn update_from(&mut self, other: &CommandHints) {
        for (key, value) in &other.cache {
            if !self.cache.contains_key(key) {
                self.cache.insert(key.clone(), value.clone());
            }
        }
    }

    fn format_inline_hint(&self, doc: &CommandDoc, arg_index: usize) -> Option<String> {
        if doc.arguments.is_empty() {
            return Some(format!("{} | {}", doc.summary, doc.group));
        }

        if arg_index < doc.arguments.len() {
            let arg = &doc.arguments[arg_index];
            let optional_mark = if arg.optional { " [opt]" } else { "" };
            let multiple_mark = if arg.multiple { " [+]" } else { "" };
            return Some(format!(
                "arg{}: {}{}{} | {}",
                arg_index + 1,
                arg.name, 
                optional_mark, 
                multiple_mark,
                doc.summary
            ));
        }

        Some(format!("{} | {}", doc.summary, doc.group))
    }
}

impl Default for CommandHints {
    fn default() -> Self {
        Self::new()
    }
}

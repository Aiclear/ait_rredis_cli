use std::collections::HashMap;

use crate::redis_client::RedisClient;
use crate::redis_type::{Map, RespType};

pub struct CommandCompleter {
    command_docs_cache: HashMap<String, CommandDoc>,
}

#[derive(Debug, Clone)]
pub struct CommandDoc {
    pub name: String,
    pub summary: Option<String>,
    pub since: Option<String>,
    pub group: Option<String>,
    pub complexity: Option<String>,
    pub doc_flags: Option<Vec<String>>,
    pub arguments: Vec<ArgumentInfo>,
}

#[derive(Debug, Clone)]
pub struct ArgumentInfo {
    pub name: String,
    pub optional: bool,
    pub multiple: bool,
}

impl CommandCompleter {
    pub fn new() -> Self {
        Self {
            command_docs_cache: HashMap::new(),
        }
    }

    pub fn get_command_doc(&mut self, redis_client: &mut RedisClient, command: &str) -> Option<CommandDoc> {
        let cmd_lower = command.to_lowercase();
        
        if let Some(doc) = self.command_docs_cache.get(&cmd_lower) {
            return Some(doc.clone());
        }

        let resp = RespType::create_from_command_line(&format!("COMMAND DOCS {}", cmd_lower));
        if redis_client.write_command(resp).is_err() {
            return None;
        }

        let response = redis_client.read_resp().ok()?;
        let doc = Self::parse_command_docs(&response)?;
        
        self.command_docs_cache.insert(cmd_lower, doc.clone());
        Some(doc)
    }

    fn parse_command_docs(resp: &RespType) -> Option<CommandDoc> {
        match resp {
            RespType::Maps(map) => {
                Self::parse_from_top_level_map(map)
            }
            RespType::Arrays(arr) => {
                if arr.is_empty() {
                    return None;
                }
                
                let first = arr.get(0)?;
                if let RespType::BulkStrings(bs) = first {
                    let name = bs.value();
                    if arr.len() > 1 {
                        if let RespType::Maps(inner_map) = arr.get(1)? {
                            return Self::parse_from_command_map(name, inner_map);
                        }
                    }
                }
                
                if let RespType::Arrays(cmd_info) = first {
                    Self::parse_single_command_doc(cmd_info.as_slice())
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn parse_from_top_level_map(map: &Map) -> Option<CommandDoc> {
        for (key, value) in map.iter() {
            if let RespType::BulkStrings(bs) = key {
                let cmd_name = bs.value();
                if let RespType::Maps(inner_map) = value {
                    return Self::parse_from_command_map(cmd_name, inner_map);
                }
            }
        }
        None
    }

    fn parse_from_command_map(name: &str, map: &Map) -> Option<CommandDoc> {
        let mut doc = CommandDoc {
            name: name.to_string(),
            summary: None,
            since: None,
            group: None,
            complexity: None,
            doc_flags: None,
            arguments: Vec::new(),
        };

        for (key, value) in map.iter() {
            let key_str = Self::extract_string_from_resp(key);
            if let Some(key_str) = key_str {
                match key_str.to_lowercase().as_str() {
                    "summary" => {
                        doc.summary = Self::extract_string_from_resp(value);
                    }
                    "since" => {
                        doc.since = Self::extract_string_from_resp(value);
                    }
                    "group" => {
                        doc.group = Self::extract_string_from_resp(value);
                    }
                    "complexity" => {
                        doc.complexity = Self::extract_string_from_resp(value);
                    }
                    "doc_flags" => {
                        doc.doc_flags = Self::extract_string_array(value);
                    }
                    "arguments" => {
                        doc.arguments = Self::parse_arguments(value);
                    }
                    _ => {}
                }
            }
        }

        Some(doc)
    }

    fn parse_single_command_doc(cmd_info: &[RespType]) -> Option<CommandDoc> {
        let name = Self::extract_string_from_resp(cmd_info.get(0)?)?;
        
        let mut doc = CommandDoc {
            name: name.clone(),
            summary: None,
            since: None,
            group: None,
            complexity: None,
            doc_flags: None,
            arguments: Vec::new(),
        };

        let mut i = 1;
        while i + 1 < cmd_info.len() {
            let key = Self::extract_string_from_resp(&cmd_info[i]);
            let value = &cmd_info[i + 1];
            
            if let Some(key_str) = key {
                match key_str.to_lowercase().as_str() {
                    "summary" => {
                        doc.summary = Self::extract_string_from_resp(value);
                    }
                    "since" => {
                        doc.since = Self::extract_string_from_resp(value);
                    }
                    "group" => {
                        doc.group = Self::extract_string_from_resp(value);
                    }
                    "complexity" => {
                        doc.complexity = Self::extract_string_from_resp(value);
                    }
                    "doc_flags" => {
                        doc.doc_flags = Self::extract_string_array(value);
                    }
                    "arguments" => {
                        doc.arguments = Self::parse_arguments(value);
                    }
                    _ => {}
                }
            }
            i += 2;
        }

        Some(doc)
    }

    fn extract_string_from_resp(resp: &RespType) -> Option<String> {
        match resp {
            RespType::BulkStrings(bs) => Some(bs.value().to_string()),
            RespType::SimpleStrings(ss) => Some(ss.value().to_string()),
            _ => None,
        }
    }

    fn extract_string_array(resp: &RespType) -> Option<Vec<String>> {
        match resp {
            RespType::Arrays(arr) => {
                let mut result = Vec::new();
                for item in arr.iter() {
                    if let Some(s) = Self::extract_string_from_resp(item) {
                        result.push(s);
                    }
                }
                Some(result)
            }
            RespType::Sets(set) => {
                let mut result = Vec::new();
                for item in set.iter() {
                    if let Some(s) = Self::extract_string_from_resp(item) {
                        result.push(s);
                    }
                }
                Some(result)
            }
            _ => None,
        }
    }

    fn parse_arguments(resp: &RespType) -> Vec<ArgumentInfo> {
        let mut args = Vec::new();
        
        if let RespType::Arrays(arr) = resp {
            for arg_resp in arr.iter() {
                if let RespType::Arrays(arg_info) = arg_resp {
                    let mut arg = ArgumentInfo {
                        name: String::new(),
                        optional: false,
                        multiple: false,
                    };

                    let slice = arg_info.as_slice();
                    let mut i = 0;
                    while i + 1 < slice.len() {
                        let key = Self::extract_string_from_resp(&slice[i]);
                        let value = &slice[i + 1];
                        
                        if let Some(key_str) = key {
                            match key_str.to_lowercase().as_str() {
                                "name" => {
                                    arg.name = Self::extract_string_from_resp(value)
                                        .unwrap_or_default();
                                }
                                "optional" => {
                                    arg.optional = Self::extract_string_from_resp(value)
                                        .map(|s| s == "true")
                                        .unwrap_or(false);
                                }
                                "multiple" => {
                                    arg.multiple = Self::extract_string_from_resp(value)
                                        .map(|s| s == "true")
                                        .unwrap_or(false);
                                }
                                _ => {}
                            }
                        }
                        i += 2;
                    }

                    if !arg.name.is_empty() {
                        args.push(arg);
                    }
                }
            }
        }

        args
    }

    pub fn format_help(doc: &CommandDoc) -> String {
        let mut help = String::new();
        
        help.push_str(&format!("\n{}\n", "=".repeat(60)));
        help.push_str(&format!("Command: {}\n", doc.name.to_uppercase()));
        help.push_str(&format!("{}\n", "=".repeat(60)));
        
        if let Some(summary) = &doc.summary {
            help.push_str(&format!("Summary: {}\n", summary));
        }
        
        if let Some(group) = &doc.group {
            help.push_str(&format!("Group: {}\n", group));
        }
        
        if let Some(since) = &doc.since {
            help.push_str(&format!("Since: {}\n", since));
        }
        
        if let Some(complexity) = &doc.complexity {
            help.push_str(&format!("Complexity: {}\n", complexity));
        }

        if !doc.arguments.is_empty() {
            help.push_str("\nArguments:\n");
            for arg in &doc.arguments {
                let mut arg_str = format!("  {}", arg.name);
                if arg.optional {
                    arg_str.push_str(" (optional)");
                }
                if arg.multiple {
                    arg_str.push_str(" (multiple)");
                }
                help.push_str(&format!("{}\n", arg_str));
            }
        }
        
        help.push_str(&format!("{}\n", "=".repeat(60)));
        
        help
    }

    pub fn get_suggestions(&mut self, redis_client: &mut RedisClient, input: &str) -> Vec<String> {
        let parts: Vec<&str> = input.split_whitespace().collect();
        
        if parts.is_empty() {
            return Vec::new();
        }

        let cmd = parts[0].to_lowercase();
        
        if parts.len() == 1 {
            let doc = self.get_command_doc(redis_client, &cmd);
            if let Some(doc) = doc {
                let mut suggestions = Vec::new();
                
                if !doc.arguments.is_empty() {
                    let next_arg = &doc.arguments[0];
                    let mut hint = if next_arg.optional {
                        format!("[{}]", next_arg.name)
                    } else {
                        format!("<{}>", next_arg.name)
                    };
                    
                    if next_arg.multiple {
                        hint.push_str(" ...");
                    }
                    
                    suggestions.push(hint);
                }
                
                suggestions.push(Self::format_help(&doc));
                
                return suggestions;
            }
        } else if parts.len() > 1 {
            let doc = self.get_command_doc(redis_client, &cmd);
            if let Some(doc) = doc {
                let arg_idx = parts.len() - 1;
                
                if arg_idx <= doc.arguments.len() {
                    let arg = &doc.arguments[arg_idx.min(doc.arguments.len() - 1)];
                    let mut hint = if arg.optional {
                        format!("[{}]", arg.name)
                    } else {
                        format!("<{}>", arg.name)
                    };
                    
                    if arg.multiple {
                        hint.push_str(" (can repeat)");
                    }
                    
                    return vec![hint];
                }
            }
        }

        Vec::new()
    }
}

impl Default for CommandCompleter {
    fn default() -> Self {
        Self::new()
    }
}

pub fn is_monitor_command(input: &str) -> bool {
    input.trim().to_lowercase() == "_monitor"
}

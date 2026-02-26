use std::collections::HashMap;

use crate::{
    redis_client::RedisClient,
    redis_type::RespType,
};

pub struct CommandDocs {
    cache: HashMap<String, CommandDoc>,
}

#[derive(Clone)]
pub struct CommandDoc {
    pub summary: String,
    pub arguments: Vec<ArgDoc>,
}

#[derive(Clone)]
pub struct ArgDoc {
    pub name: String,
    pub type_: String,
    pub optional: bool,
}

impl CommandDocs {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
        }
    }

    pub fn get_doc(&mut self, client: &mut RedisClient, command: &str) -> Option<&CommandDoc> {
        let command_upper = command.to_uppercase();
        
        if self.cache.contains_key(&command_upper) {
            return self.cache.get(&command_upper);
        }

        if let Ok(doc) = self.fetch_command_doc(client, &command_upper) {
            self.cache.insert(command_upper.clone(), doc);
            self.cache.get(&command_upper)
        } else {
            None
        }
    }

    fn fetch_command_doc(&self, client: &mut RedisClient, command: &str) -> anyhow::Result<CommandDoc> {
        let cmd = format!("COMMAND DOCS {}", command);
        let resp_type = RespType::create_from_command_line(&cmd);
        client.write_command(resp_type)?;
        
        let response = client.read_resp()?;
        self.parse_command_doc(&response, command)
    }

    fn parse_command_doc(&self, response: &RespType, command: &str) -> anyhow::Result<CommandDoc> {
        match response {
            RespType::Maps(map) => {
                if map.map.is_empty() {
                    return Ok(CommandDoc {
                        summary: format!("No documentation for {}", command),
                        arguments: Vec::new(),
                    });
                }
                
                for (_, value) in &map.map {
                    if let RespType::Maps(doc_map) = value {
                        let mut summary = String::new();
                        let mut arguments = Vec::new();
                        
                        for (key, val) in &doc_map.map {
                            let key_str = Self::extract_string(&key.1);
                            match key_str.as_str() {
                                "summary" => {
                                    summary = Self::extract_string(val);
                                }
                                "arguments" => {
                                    if let RespType::Arrays(arr) = val {
                                        for arg in &arr.value {
                                            if let Some(arg_doc) = Self::parse_argument(arg) {
                                                arguments.push(arg_doc);
                                            }
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                        
                        return Ok(CommandDoc { summary, arguments });
                    }
                }
                
                Ok(CommandDoc {
                    summary: format!("No documentation for {}", command),
                    arguments: Vec::new(),
                })
            }
            _ => Ok(CommandDoc {
                summary: format!("No documentation for {}", command),
                arguments: Vec::new(),
            }),
        }
    }

    fn parse_argument(arg: &RespType) -> Option<ArgDoc> {
        match arg {
            RespType::Maps(map) => {
                let mut name = String::new();
                let mut type_ = "string".to_string();
                let mut optional = false;
                
                for (key, val) in &map.map {
                    let key_str = Self::extract_string(&key.1);
                    match key_str.as_str() {
                        "name" => {
                            name = Self::extract_string(val);
                        }
                        "type" => {
                            type_ = Self::extract_string(val);
                        }
                        "optional" => {
                            if let RespType::Booleans(b) = val {
                                optional = b.value;
                            }
                        }
                        _ => {}
                    }
                }
                
                if !name.is_empty() {
                    Some(ArgDoc { name, type_, optional })
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn extract_string(resp: &RespType) -> String {
        match resp {
            RespType::SimpleStrings(s) => s.value.clone(),
            RespType::BulkStrings(b) => b.value.clone(),
            _ => String::new(),
        }
    }

    pub fn format_hint(&self, doc: &CommandDoc) -> String {
        Self::format_hint_static(doc)
    }

    pub fn format_hint_static(doc: &CommandDoc) -> String {
        if doc.arguments.is_empty() {
            return doc.summary.clone();
        }

        let args: Vec<String> = doc.arguments
            .iter()
            .map(|arg| {
                if arg.optional {
                    format!("[{}]", arg.name)
                } else {
                    format!("<{}>", arg.name)
                }
            })
            .collect();

        format!("{} - Arguments: {}", doc.summary, args.join(" "))
    }

    pub fn get_commands_list(&mut self, _client: &mut RedisClient) -> Vec<String> {
        static COMMON_COMMANDS: &[&str] = &[
            "GET", "SET", "DEL", "EXISTS", "KEYS", "EXPIRE", "TTL", "TYPE", "RENAME",
            "HGET", "HSET", "HDEL", "HEXISTS", "HGETALL", "HKEYS", "HLEN", "HVALS",
            "LPUSH", "RPUSH", "LPOP", "RPOP", "LLEN", "LRANGE", "LSET",
            "SADD", "SREM", "SMEMBERS", "SISMEMBER", "SCARD", "SUNION", "SINTER",
            "ZADD", "ZRANGE", "ZREM", "ZCARD", "ZSCORE",
            "SELECT", "INFO", "PING", "QUIT", "FLUSHDB", "FLUSHALL", "DBSIZE",
            "MONITOR", "CLIENT", "CONFIG", "SAVE", "BGSAVE", "LASTSAVE",
        ];
        
        COMMON_COMMANDS.iter().map(|s| s.to_string()).collect()
    }
}

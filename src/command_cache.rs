use crate::redis_client::RedisClient;
use crate::redis_type::RespType;
use std::collections::HashMap;
use std::time::Instant;

#[derive(Debug, Clone)]
pub struct CommandInfo {
    pub name: String,
    pub arity: i32,
    pub flags: Vec<String>,
    pub first_key: i32,
    pub last_key: i32,
    pub step: i32,
    pub key_specs: Vec<KeySpec>,
    pub subcommands: Vec<String>,
    pub tips: Vec<String>,
    pub doc_table: Vec<Vec<String>>,
}

#[derive(Debug, Clone)]
pub struct KeySpec {
    pub flags: Vec<String>,
    pub start_search: KeySearch,
    pub find_keys: KeyFind,
}

#[derive(Debug, Clone)]
pub enum KeySearch {
    Index(i32),
    Keyword(String),
    Unknown,
}

#[derive(Debug, Clone)]
pub enum KeyFind {
    Range(i32, i32),
    KeyNum(i32),
    KeyNumPlus(i32),
    Unknown,
}

pub struct CommandCache {
    commands: HashMap<String, CommandInfo>,
    keys: Vec<String>,
    last_keys_update: Instant,
}

impl CommandCache {
    pub fn new() -> Self {
        Self {
            commands: HashMap::new(),
            keys: Vec::new(),
            last_keys_update: Instant::now(),
        }
    }

    pub fn fetch_command_docs(&mut self, client: &mut RedisClient) -> anyhow::Result<()> {
        // 获取所有命令的基本信息
        let command_list_resp = client.execute_command("COMMAND")?;

        if let RespType::Arrays(commands) = command_list_resp {
            for cmd in commands.value {
                if let RespType::Arrays(cmd_info) = cmd {
                    if cmd_info.value.len() >= 6 {
                        if let RespType::BulkStrings(name_bulk) = &cmd_info.value[0] {
                            let name = name_bulk.value.to_uppercase();

                            let command_info = CommandInfo {
                                name: name.clone(),
                                arity: if let RespType::Integers(n) = &cmd_info.value[1] {
                                    n.value as i32
                                } else {
                                    0
                                },
                                flags: self.extract_string_array(&cmd_info.value[2]),
                                first_key: if let RespType::Integers(n) = &cmd_info.value[3] {
                                    n.value as i32
                                } else {
                                    0
                                },
                                last_key: if let RespType::Integers(n) = &cmd_info.value[4] {
                                    n.value as i32
                                } else {
                                    0
                                },
                                step: if let RespType::Integers(n) = &cmd_info.value[5] {
                                    n.value as i32
                                } else {
                                    0
                                },
                                key_specs: Vec::new(),
                                subcommands: Vec::new(),
                                tips: Vec::new(),
                                doc_table: Vec::new(),
                            };

                            self.commands.insert(name, command_info);
                        }
                    }
                }
            }
        }

        // 获取详细文档信息
        self.fetch_detailed_docs(client)?;

        Ok(())
    }

    fn fetch_detailed_docs(&mut self, client: &mut RedisClient) -> anyhow::Result<()> {
        // 对每个命令获取详细文档
        let command_names: Vec<String> = self.commands.keys().cloned().collect();

        for command_name in command_names {
            let doc_command = format!("COMMAND DOC {}", command_name);
            match client.execute_command(&doc_command) {
                Ok(doc_resp) => {
                    // 解析文档然后更新，避免借用冲突
                    let parsed_doc = self.parse_command_doc_response(doc_resp);
                    if let Some(doc_info) = parsed_doc {
                        if let Some(cmd_info) = self.commands.get_mut(&command_name) {
                            cmd_info.doc_table = doc_info.doc_table;
                            cmd_info.subcommands = doc_info.subcommands;
                        }
                    }
                }
                Err(_) => {
                    // 如果COMMAND DOC不支持，跳过详细文档
                    continue;
                }
            }
        }

        Ok(())
    }

    fn parse_command_doc_response(&self, doc_resp: RespType) -> Option<CommandInfo> {
        let mut cmd_info = CommandInfo {
            name: String::new(),
            arity: 0,
            flags: Vec::new(),
            first_key: 0,
            last_key: 0,
            step: 0,
            key_specs: Vec::new(),
            subcommands: Vec::new(),
            tips: Vec::new(),
            doc_table: Vec::new(),
        };

        self.parse_command_doc(doc_resp, &mut cmd_info);
        Some(cmd_info)
    }

    fn parse_command_doc(&self, doc_resp: RespType, cmd_info: &mut CommandInfo) {
        // 解析COMMAND DOC的响应
        if let RespType::Arrays(doc_data) = doc_resp {
            if doc_data.value.len() >= 3 {
                // doc_data通常包含: [command_name, doc_table, subcommands]
                if let RespType::Arrays(doc_table) = &doc_data.value[1] {
                    for row in &doc_table.value {
                        if let RespType::Arrays(row_data) = row {
                            let row_strings: Vec<String> = row_data
                                .value
                                .iter()
                                .map(|cell| self.extract_string(cell))
                                .collect();
                            cmd_info.doc_table.push(row_strings);
                        }
                    }
                }

                if let RespType::Arrays(subcommands) = &doc_data.value[2] {
                    for subcmd in &subcommands.value {
                        if let RespType::BulkStrings(name_bytes) = subcmd {
                            cmd_info.subcommands.push(name_bytes.value.clone());
                        }
                    }
                }
            }
        }
    }

    pub fn update_keys(&mut self, client: &mut RedisClient) -> anyhow::Result<()> {
        // 每30秒更新一次keys缓存
        if self.last_keys_update.elapsed().as_secs() < 30 {
            return Ok(());
        }

        match client.execute_command("KEYS *") {
            Ok(keys_resp) => {
                if let RespType::Arrays(keys_array) = keys_resp {
                    self.keys.clear();
                    for key in keys_array.value {
                        if let RespType::BulkStrings(key_bytes) = key {
                            self.keys.push(key_bytes.value.clone());
                        }
                    }
                }
                self.last_keys_update = Instant::now();
            }
            Err(_) => {
                // 如果KEYS命令失败，保持现有keys
            }
        }

        Ok(())
    }

    pub fn get_command(&self, name: &str) -> Option<&CommandInfo> {
        self.commands.get(&name.to_uppercase())
    }

    pub fn get_matching_commands(&self, prefix: &str) -> Vec<String> {
        let prefix_upper = prefix.to_uppercase();
        self.commands
            .keys()
            .filter(|cmd| cmd.starts_with(&prefix_upper))
            .cloned()
            .collect()
    }

    pub fn get_matching_keys(&self, prefix: &str) -> Vec<String> {
        self.keys
            .iter()
            .filter(|key| key.starts_with(prefix))
            .cloned()
            .collect()
    }

    // 辅助方法
    fn extract_string_array(&self, resp: &RespType) -> Vec<String> {
        if let RespType::Arrays(arr) = resp {
            arr.value
                .iter()
                .map(|item| self.extract_string(item))
                .collect()
        } else {
            Vec::new()
        }
    }

    fn extract_string(&self, resp: &RespType) -> String {
        match resp {
            RespType::BulkStrings(bytes) => bytes.value.clone(),
            RespType::SimpleStrings(s) => s.value.clone(),
            _ => String::new(),
        }
    }
}

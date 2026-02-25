use crate::command_cache::CommandCache;
use rustyline::{
    completion::Completer, highlight::Highlighter, hint::Hinter, validate::Validator, Context,
    Result,
};
use std::sync::{Arc, Mutex};

pub struct SmartCompleter {
    cache: Arc<Mutex<CommandCache>>,
}

impl SmartCompleter {
    pub fn new(cache: Arc<Mutex<CommandCache>>) -> Self {
        Self { cache }
    }

    fn parse_command_line(&self, line: &str) -> (String, Vec<String>, usize) {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.is_empty() {
            return (String::new(), Vec::new(), 0);
        }

        let command = parts[0].to_uppercase();
        let args: Vec<String> = parts[1..].iter().map(|s| s.to_string()).collect();

        // 计算当前参数位置
        let current_pos = if line.ends_with(' ') {
            args.len()
        } else {
            args.len().saturating_sub(1)
        };

        (command, args, current_pos)
    }

    fn get_command_completions(&self, prefix: &str) -> Vec<String> {
        let cache = self.cache.lock().unwrap();
        cache.get_matching_commands(prefix)
    }

    fn get_key_completions(&self, prefix: &str) -> Vec<String> {
        let cache = self.cache.lock().unwrap();
        cache.get_matching_keys(prefix)
    }

    fn get_parameter_completions(
        &self,
        command: &str,
        args: &[String],
        current_pos: usize,
        prefix: &str,
    ) -> Vec<String> {
        let cache = self.cache.lock().unwrap();

        if let Some(_cmd_info) = cache.get_command(command) {
            // 根据命令类型提供不同的参数补全
            match command {
                "GET" | "SET" | "DEL" | "EXISTS" | "TYPE" | "TTL" | "EXPIRE" | "HGET" | "HSET"
                | "HDEL" | "HGETALL" => {
                    // 这些命令的第一个参数是key
                    if current_pos == 0 {
                        return self.get_key_completions(prefix);
                    }
                }
                "LPUSH" | "RPUSH" | "LPOP" | "RPOP" | "LLEN" => {
                    // List相关命令
                    if current_pos == 0 {
                        return self.get_key_completions(prefix);
                    }
                }
                "SADD" | "SREM" | "SMEMBERS" | "SCARD" => {
                    // Set相关命令
                    if current_pos == 0 {
                        return self.get_key_completions(prefix);
                    }
                }
                "ZADD" | "ZREM" | "ZRANGE" | "ZCARD" => {
                    // Sorted Set相关命令
                    if current_pos == 0 {
                        return self.get_key_completions(prefix);
                    }
                }
                "CONFIG" => {
                    // CONFIG命令的子命令补全
                    if current_pos == 0 {
                        return vec![
                            "GET".to_string(),
                            "SET".to_string(),
                            "RESETSTAT".to_string(),
                        ];
                    } else if current_pos == 1 && args.get(0).map(|s| s.as_str()) == Some("GET") {
                        return vec![
                            "*".to_string(),
                            "maxmemory".to_string(),
                            "timeout".to_string(),
                            "save".to_string(),
                        ];
                    }
                }
                "INFO" => {
                    // INFO命令的参数补全
                    return vec![
                        "".to_string(),
                        "server".to_string(),
                        "clients".to_string(),
                        "memory".to_string(),
                        "persistence".to_string(),
                        "stats".to_string(),
                        "replication".to_string(),
                        "cpu".to_string(),
                        "commandstats".to_string(),
                        "cluster".to_string(),
                        "keyspace".to_string(),
                    ];
                }
                "KEYS" => {
                    // KEYS命令的模式补全
                    return vec![
                        "*".to_string(),
                        "user:*".to_string(),
                        "session:*".to_string(),
                        "cache:*".to_string(),
                    ];
                }
                _ => {
                    // 对于其他命令，提供基本参数提示
                    if current_pos == 0 {
                        return vec!["<key>".to_string()];
                    }
                }
            }
        }

        Vec::new()
    }

    fn get_value_completions(&self, command: &str, args: &[String], _prefix: &str) -> Vec<String> {
        // 根据命令和已有参数提供值补全
        match command {
            "SET" => {
                if args.len() == 1 {
                    // SET命令的值补全建议
                    return vec![
                        "\"value\"".to_string(),
                        "123".to_string(),
                        "true".to_string(),
                        "false".to_string(),
                    ];
                } else if args.len() >= 2 {
                    // SET命令的选项补全
                    return vec![
                        "EX".to_string(),
                        "PX".to_string(),
                        "NX".to_string(),
                        "XX".to_string(),
                    ];
                }
            }
            "EXPIRE" => {
                if args.len() == 1 {
                    // EXPIRE命令的时间补全
                    return vec![
                        "60".to_string(),
                        "300".to_string(),
                        "3600".to_string(),
                        "86400".to_string(),
                    ];
                }
            }
            "CONFIG" => {
                if args.len() == 2 && args.get(0).map(|s| s.as_str()) == Some("SET") {
                    // CONFIG SET的值补全
                    match args.get(1).map(|s| s.as_str()) {
                        Some("maxmemory") => {
                            return vec![
                                "1gb".to_string(),
                                "512mb".to_string(),
                                "256mb".to_string(),
                            ]
                        }
                        Some("timeout") => {
                            return vec!["300".to_string(), "600".to_string(), "0".to_string()]
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }

        Vec::new()
    }
}

impl Completer for SmartCompleter {
    type Candidate = String;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &Context<'_>,
    ) -> Result<(usize, Vec<Self::Candidate>)> {
        let (command, args, current_pos) = self.parse_command_line(&line[..pos]);

        // 确定补全的起始位置
        let start = if let Some(last_space) = line[..pos].rfind(' ') {
            last_space + 1
        } else {
            0
        };

        let current_input = &line[start..pos];

        let completions = if command.is_empty() {
            // 没有输入命令，提供命令补全
            self.get_command_completions(current_input)
        } else {
            // 有命令，提供参数补全
            if current_pos == 0 {
                // 第一个参数，通常是key
                self.get_parameter_completions(&command, &args, current_pos, current_input)
            } else {
                // 后续参数，可能是值或选项
                let mut completions = self.get_value_completions(&command, &args, current_input);

                if completions.is_empty() {
                    // 如果没有特定的值补全，尝试参数补全
                    completions =
                        self.get_parameter_completions(&command, &args, current_pos, current_input);
                }
                completions
            }
        };

        // 过滤匹配当前输入的补全项
        let filtered: Vec<String> = completions
            .into_iter()
            .filter(|candidate: &String| candidate.starts_with(current_input))
            .collect();

        Ok((start, filtered))
    }
}

impl Hinter for SmartCompleter {
    type Hint = String;
}

impl Highlighter for SmartCompleter {}

impl Validator for SmartCompleter {}

impl rustyline::Helper for SmartCompleter {}

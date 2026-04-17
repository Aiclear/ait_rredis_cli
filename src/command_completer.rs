use std::collections::HashMap;
use std::sync::Arc;
use rustyline::completion::{Completer, Pair};
use rustyline::hint::{Hinter, HistoryHinter};
use rustyline::highlight::{Highlighter, MatchingBracketHighlighter};
use rustyline::validate::{Validator, MatchingBracketValidator};
use rustyline::Context;
use rustyline_derive::Helper;

use crate::redis_client::RedisClient;
use crate::redis_type::RespType;

#[derive(Debug, Clone)]
pub struct RedisCommand {
    pub name: String,
    pub summary: String,
    pub complexity: String,
    pub arguments: Vec<RedisCommandArgument>,
}

#[derive(Debug, Clone)]
pub struct RedisCommandArgument {
    pub name: String,
    pub arg_type: String,
    pub optional: bool,
    pub multiple: bool,
}

pub struct RedisCommandCompleter {
    command_names: Vec<String>,
    command_cache: HashMap<String, RedisCommand>,
    redis_client: Option<Arc<std::sync::Mutex<RedisClient>>>,
    builtin_commands: HashMap<String, RedisCommand>,
}

impl RedisCommandCompleter {
    pub fn new() -> Self {
        let mut builtin_commands = HashMap::new();
        
        // Add some built-in commands as fallback
        Self::add_builtin_command(&mut builtin_commands, 
            "get", "Get the value of a key", "O(1)",
            vec![("key", "key", false, false)]
        );
        Self::add_builtin_command(&mut builtin_commands,
            "set", "Set the string value of a key", "O(1)",
            vec![("key", "key", false, false), ("value", "string", false, false)]
        );
        Self::add_builtin_command(&mut builtin_commands,
            "del", "Delete a key", "O(1)",
            vec![("key", "key", false, true)]
        );
        Self::add_builtin_command(&mut builtin_commands,
            "exists", "Determine if a key exists", "O(1)",
            vec![("key", "key", false, true)]
        );
        Self::add_builtin_command(&mut builtin_commands,
            "keys", "Find all keys matching the given pattern", "O(N)",
            vec![("pattern", "pattern", false, false)]
        );
        Self::add_builtin_command(&mut builtin_commands,
            "ping", "Ping the server", "O(1)",
            vec![]
        );
        Self::add_builtin_command(&mut builtin_commands,
            "info", "Get information and statistics about the server", "O(1)",
            vec![("section", "string", true, false)]
        );
        Self::add_builtin_command(&mut builtin_commands,
            "config", "Get or set server configuration", "O(1)",
            vec![("subcommand", "string", false, false)]
        );
        Self::add_builtin_command(&mut builtin_commands,
            "client", "Manage client connections", "O(1)",
            vec![("subcommand", "string", false, false)]
        );
        Self::add_builtin_command(&mut builtin_commands,
            "monitor", "Listen for all requests received by the server", "O(1)",
            vec![]
        );
        Self::add_builtin_command(&mut builtin_commands,
            "quit", "Close the connection", "O(1)",
            vec![]
        );
        Self::add_builtin_command(&mut builtin_commands,
            "select", "Change the selected database for the current connection", "O(1)",
            vec![("index", "integer", false, false)]
        );
        Self::add_builtin_command(&mut builtin_commands,
            "incr", "Increment the integer value of a key by one", "O(1)",
            vec![("key", "key", false, false)]
        );
        Self::add_builtin_command(&mut builtin_commands,
            "decr", "Decrement the integer value of a key by one", "O(1)",
            vec![("key", "key", false, false)]
        );
        Self::add_builtin_command(&mut builtin_commands,
            "lpush", "Prepend one or multiple values to a list", "O(1)",
            vec![("key", "key", false, false), ("element", "string", false, true)]
        );
        Self::add_builtin_command(&mut builtin_commands,
            "rpush", "Append one or multiple values to a list", "O(1)",
            vec![("key", "key", false, false), ("element", "string", false, true)]
        );
        Self::add_builtin_command(&mut builtin_commands,
            "lpop", "Remove and get the first element in a list", "O(1)",
            vec![("key", "key", false, false)]
        );
        Self::add_builtin_command(&mut builtin_commands,
            "rpop", "Remove and get the last element in a list", "O(1)",
            vec![("key", "key", false, false)]
        );
        Self::add_builtin_command(&mut builtin_commands,
            "llen", "Get the length of a list", "O(1)",
            vec![("key", "key", false, false)]
        );
        Self::add_builtin_command(&mut builtin_commands,
            "hset", "Set the string value of a hash field", "O(1)",
            vec![("key", "key", false, false), ("field", "string", false, false), ("value", "string", false, false)]
        );
        Self::add_builtin_command(&mut builtin_commands,
            "hget", "Get the value of a hash field", "O(1)",
            vec![("key", "key", false, false), ("field", "string", false, false)]
        );
        Self::add_builtin_command(&mut builtin_commands,
            "hgetall", "Get all the fields and values in a hash", "O(N)",
            vec![("key", "key", false, false)]
        );
        Self::add_builtin_command(&mut builtin_commands,
            "hkeys", "Get all the fields in a hash", "O(N)",
            vec![("key", "key", false, false)]
        );
        Self::add_builtin_command(&mut builtin_commands,
            "hvals", "Get all the values in a hash", "O(N)",
            vec![("key", "key", false, false)]
        );
        Self::add_builtin_command(&mut builtin_commands,
            "sadd", "Add one or more members to a set", "O(1)",
            vec![("key", "key", false, false), ("member", "string", false, true)]
        );
        Self::add_builtin_command(&mut builtin_commands,
            "smembers", "Get all the members in a set", "O(N)",
            vec![("key", "key", false, false)]
        );
        Self::add_builtin_command(&mut builtin_commands,
            "sismember", "Determine if a given value is a member of a set", "O(1)",
            vec![("key", "key", false, false), ("member", "string", false, false)]
        );
        Self::add_builtin_command(&mut builtin_commands,
            "scard", "Get the number of members in a set", "O(1)",
            vec![("key", "key", false, false)]
        );
        Self::add_builtin_command(&mut builtin_commands,
            "zadd", "Add one or more members to a sorted set, or update its score", "O(log(N))",
            vec![("key", "key", false, false), ("score", "double", false, false), ("member", "string", false, false)]
        );
        Self::add_builtin_command(&mut builtin_commands,
            "zrange", "Return a range of members in a sorted set, by index", "O(log(N)+M)",
            vec![("key", "key", false, false), ("start", "integer", false, false), ("stop", "integer", false, false)]
        );
        Self::add_builtin_command(&mut builtin_commands,
            "zrangebyscore", "Return a range of members in a sorted set, by score", "O(log(N)+M)",
            vec![("key", "key", false, false), ("min", "string", false, false), ("max", "string", false, false)]
        );

        let command_names: Vec<String> = builtin_commands.keys().cloned().collect();

        Self {
            command_names,
            command_cache: HashMap::new(),
            redis_client: None,
            builtin_commands,
        }
    }

    fn add_builtin_command(
        builtin_commands: &mut HashMap<String, RedisCommand>,
        name: &str,
        summary: &str,
        complexity: &str,
        args: Vec<(&str, &str, bool, bool)>,
    ) {
        let arguments: Vec<RedisCommandArgument> = args
            .into_iter()
            .map(|(name, arg_type, optional, multiple)| RedisCommandArgument {
                name: name.to_string(),
                arg_type: arg_type.to_string(),
                optional,
                multiple,
            })
            .collect();

        let command = RedisCommand {
            name: name.to_string(),
            summary: summary.to_string(),
            complexity: complexity.to_string(),
            arguments,
        };

        builtin_commands.insert(name.to_string(), command);
    }

    pub fn set_redis_client(&mut self, client: Arc<std::sync::Mutex<RedisClient>>) {
        self.redis_client = Some(client);
        
        // Try to get command list from server
        if let Err(e) = self.fetch_commands_from_server() {
            eprintln!("Warning: Could not fetch commands from server: {}", e);
            eprintln!("Using built-in commands as fallback");
        }
    }

    fn fetch_commands_from_server(&mut self) -> anyhow::Result<()> {
        if let Some(client) = &self.redis_client {
            let mut client = client.lock().unwrap();
            
            // Try COMMAND LIST first (lighter)
            let cmd = RespType::create_from_command_line("COMMAND LIST");
            client.write_command(cmd)?;
            let response = client.read_resp()?;
            
            if !response.is_err_type() {
                if let Some(array) = response.as_array() {
                    let mut new_commands = Vec::new();
                    for item in array.iter() {
                        if let Some(bs) = item.as_bulk_string() {
                            let cmd_name = bs.value().to_lowercase();
                            if !self.builtin_commands.contains_key(&cmd_name) {
                                new_commands.push(cmd_name.clone());
                            }
                            if !self.command_names.contains(&cmd_name) {
                                self.command_names.push(cmd_name);
                            }
                        }
                    }
                    self.command_names.sort();
                }
            }
        }
        Ok(())
    }

    pub fn fetch_command_info(&mut self, command_name: &str) -> Option<&RedisCommand> {
        let cmd_name = command_name.to_lowercase();
        
        // Check cache first
        if self.command_cache.contains_key(&cmd_name) {
            return self.command_cache.get(&cmd_name);
        }
        
        // Check built-in commands
        if let Some(cmd) = self.builtin_commands.get(&cmd_name) {
            self.command_cache.insert(cmd_name.clone(), cmd.clone());
            return self.command_cache.get(&cmd_name);
        }
        
        // Try to fetch from server
        if let Some(client) = &self.redis_client {
            let mut client = client.lock().unwrap();
            
            // Try COMMAND INFO
            let cmd = RespType::create_from_command_line(&format!("COMMAND INFO {}", command_name));
            let _ = client.write_command(cmd);
            if let Ok(response) = client.read_resp() {
                if !response.is_err_type() {
                    if let Some(array) = response.as_array() {
                        for item in array.iter() {
                            if let Some(cmd_info) = Self::parse_command_info(item) {
                                self.command_cache.insert(cmd_name.clone(), cmd_info);
                                return self.command_cache.get(&cmd_name);
                            }
                        }
                    }
                }
            }
        }
        
        None
    }

    fn parse_command_info(resp: &RespType) -> Option<RedisCommand> {
        if let Some(array) = resp.as_array() {
            if array.len() >= 2 {
                // COMMAND INFO returns an array of arrays
                // Each sub-array has the structure: [name, arity, flags, first-key, last-key, step]
                // Or in Redis 6+, it may return a map
                if let Some(name_bs) = array.iter().next().and_then(|r| r.as_bulk_string()) {
                    let name = name_bs.value().to_lowercase();
                    return Some(RedisCommand {
                        name,
                        summary: String::new(),
                        complexity: String::new(),
                        arguments: Vec::new(),
                    });
                }
            }
        }
        
        None
    }

    pub fn get_command(&self, name: &str) -> Option<&RedisCommand> {
        let name_lower = name.to_lowercase();
        self.command_cache.get(&name_lower)
            .or_else(|| self.builtin_commands.get(&name_lower))
    }

    pub fn get_completions(&self, line: &str, pos: usize) -> Vec<Pair> {
        let start = if pos == 0 {
            0
        } else {
            line[..pos].rfind(' ').map_or(0, |i| i + 1)
        };
        let partial = &line[start..pos].to_lowercase();

        let mut completions = Vec::new();

        // Check if we're completing a command or arguments
        let parts: Vec<&str> = line[..pos].split_whitespace().collect();
        
        if parts.len() == 1 && !line.ends_with(' ') {
            // Completing command name
            for cmd_name in &self.command_names {
                if cmd_name.starts_with(partial) {
                    let display = if let Some(cmd) = self.get_command(cmd_name) {
                        if !cmd.summary.is_empty() {
                            format!("{} - {}", cmd_name, cmd.summary)
                        } else {
                            cmd_name.clone()
                        }
                    } else {
                        cmd_name.clone()
                    };
                    completions.push(Pair {
                        display,
                        replacement: cmd_name.clone(),
                    });
                }
            }
        }

        completions
    }

    pub fn get_hint(&self, line: &str) -> Option<String> {
        let parts: Vec<&str> = line.split_whitespace().collect();
        
        if parts.is_empty() {
            return None;
        }

        let cmd_name = parts[0].to_lowercase();
        
        if let Some(cmd) = self.get_command(&cmd_name) {
            if !cmd.arguments.is_empty() {
                // Calculate arg_index with proper boundary checks
                let arg_index: isize = if line.ends_with(' ') {
                    // User has entered command and a space, now entering first/next argument
                    parts.len() as isize - 1
                } else if parts.len() >= 2 {
                    // User is entering the nth argument (not yet completed)
                    parts.len() as isize - 2
                } else {
                    // User is still entering the command name, no parameter hint needed
                    return None;
                };

                // Ensure arg_index is non-negative
                if arg_index >= 0 && (arg_index as usize) < cmd.arguments.len() {
                    let arg = &cmd.arguments[arg_index as usize];
                    let hint = if arg.optional {
                        format!("[{}] ({})", arg.name, arg.arg_type)
                    } else {
                        format!("<{}> ({})", arg.name, arg.arg_type)
                    };
                    return Some(hint);
                }
            }
        }

        None
    }
}

#[derive(Helper)]
pub struct RedisHelper {
    completer: RedisCommandCompleter,
    highlighter: MatchingBracketHighlighter,
    validator: MatchingBracketValidator,
    hinter: HistoryHinter,
}

impl RedisHelper {
    pub fn new(completer: RedisCommandCompleter) -> Self {
        Self {
            completer,
            highlighter: MatchingBracketHighlighter::new(),
            validator: MatchingBracketValidator::new(),
            hinter: HistoryHinter {},
        }
    }

    pub fn completer(&self) -> &RedisCommandCompleter {
        &self.completer
    }

    pub fn completer_mut(&mut self) -> &mut RedisCommandCompleter {
        &mut self.completer
    }
}

impl Completer for RedisHelper {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &Context<'_>,
    ) -> rustyline::Result<(usize, Vec<Pair>)> {
        let start = if pos == 0 {
            0
        } else {
            line[..pos].rfind(' ').map_or(0, |i| i + 1)
        };
        let completions = self.completer.get_completions(line, pos);
        Ok((start, completions))
    }
}

impl Hinter for RedisHelper {
    type Hint = String;

    fn hint(&self, line: &str, _pos: usize, _ctx: &Context<'_>) -> Option<Self::Hint> {
        // First try to get hint from our completer (command arguments)
        if let Some(hint) = self.completer.get_hint(line) {
            return Some(hint);
        }
        
        // Then try history hinter
        // self.hinter.hint(line, pos, ctx)
        None
    }
}

impl Highlighter for RedisHelper {
    fn highlight_prompt<'b, 's: 'b, 'p: 'b>(
        &'s self,
        prompt: &'p str,
        default: bool,
    ) -> std::borrow::Cow<'b, str> {
        self.highlighter.highlight_prompt(prompt, default)
    }

    fn highlight_hint<'h>(&self, hint: &'h str) -> std::borrow::Cow<'h, str> {
        // Use a different color for hints
        std::borrow::Cow::Owned(format!("\x1b[90m{}\x1b[0m", hint))
    }

    fn highlight<'l>(&self, line: &'l str, pos: usize) -> std::borrow::Cow<'l, str> {
        self.highlighter.highlight(line, pos)
    }

    fn highlight_char(&self, line: &str, pos: usize, forced: bool) -> bool {
        self.highlighter.highlight_char(line, pos, forced)
    }
}

impl Validator for RedisHelper {
    fn validate(
        &self,
        ctx: &mut rustyline::validate::ValidationContext,
    ) -> rustyline::Result<rustyline::validate::ValidationResult> {
        self.validator.validate(ctx)
    }

    fn validate_while_typing(&self) -> bool {
        self.validator.validate_while_typing()
    }
}

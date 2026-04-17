use std::collections::HashMap;
use rustyline::completion::{Completer, Pair};
use rustyline::hint::{Hinter, HistoryHinter};
use rustyline::highlight::{Highlighter, MatchingBracketHighlighter};
use rustyline::validate::{Validator, MatchingBracketValidator};
use rustyline::Context;
use rustyline_derive::Helper;

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

#[derive(Debug, Clone)]
pub struct RedisCommandCompleter {
    commands: HashMap<String, RedisCommand>,
    command_names: Vec<String>,
}

impl RedisCommandCompleter {
    pub fn new() -> Self {
        Self {
            commands: HashMap::new(),
            command_names: Vec::new(),
        }
    }

    pub fn get_command_count(&self) -> usize {
        self.commands.len()
    }

    pub fn parse_command_docs(&mut self, resp: &RespType) {
        if let Some(map) = resp.as_map() {
            for (key, value) in map.iter() {
                if let Some(bs) = key.value().as_bulk_string() {
                    let command_name = bs.value().to_lowercase();
                    let mut command = RedisCommand {
                        name: command_name.clone(),
                        summary: String::new(),
                        complexity: String::new(),
                        arguments: Vec::new(),
                    };

                    if let Some(cmd_map) = value.as_map() {
                        for (cmd_key, cmd_value) in cmd_map.iter() {
                            if let Some(bs_key) = cmd_key.value().as_bulk_string() {
                                match bs_key.value() {
                                    "summary" => {
                                        if let Some(bs_val) = cmd_value.as_bulk_string() {
                                            command.summary = bs_val.value().to_string();
                                        }
                                    }
                                    "complexity" => {
                                        if let Some(bs_val) = cmd_value.as_bulk_string() {
                                            command.complexity = bs_val.value().to_string();
                                        }
                                    }
                                    "arguments" => {
                                        if let Some(args_array) = cmd_value.as_array() {
                                            for arg in args_array.iter() {
                                                if let Some(arg_map) = arg.as_map() {
                                                    let mut arg_info = RedisCommandArgument {
                                                        name: String::new(),
                                                        arg_type: String::new(),
                                                        optional: false,
                                                        multiple: false,
                                                    };
                                                    for (arg_key, arg_value) in arg_map.iter() {
                                                        if let Some(bs_arg_key) = arg_key.value().as_bulk_string() {
                                                            match bs_arg_key.value() {
                                                                "name" => {
                                                                    if let Some(bs_val) = arg_value.as_bulk_string() {
                                                                        arg_info.name = bs_val.value().to_string();
                                                                    }
                                                                }
                                                                "type" => {
                                                                    if let Some(bs_val) = arg_value.as_bulk_string() {
                                                                        arg_info.arg_type = bs_val.value().to_string();
                                                                    }
                                                                }
                                                                "optional" => {
                                                                    if let Some(b) = arg_value.as_boolean() {
                                                                        arg_info.optional = b.value();
                                                                    }
                                                                }
                                                                "multiple" => {
                                                                    if let Some(b) = arg_value.as_boolean() {
                                                                        arg_info.multiple = b.value();
                                                                    }
                                                                }
                                                                _ => {}
                                                            }
                                                        }
                                                    }
                                                    if !arg_info.name.is_empty() {
                                                        command.arguments.push(arg_info);
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }

                    self.commands.insert(command_name.clone(), command);
                    self.command_names.push(command_name);
                }
            }
            self.command_names.sort();
        }
    }

    pub fn get_command(&self, name: &str) -> Option<&RedisCommand> {
        self.commands.get(&name.to_lowercase())
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
                    if let Some(cmd) = self.commands.get(cmd_name) {
                        let display = if !cmd.summary.is_empty() {
                            format!("{} - {}", cmd_name, cmd.summary)
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
        } else if parts.len() >= 1 {
            // Completing arguments for a command
            let cmd_name = parts[0].to_lowercase();
            if let Some(cmd) = self.commands.get(&cmd_name) {
                let arg_index = if line.ends_with(' ') {
                    parts.len() - 1
                } else {
                    parts.len() - 2
                };

                if arg_index < cmd.arguments.len() {
                    let arg = &cmd.arguments[arg_index];
                    let _arg_display = if arg.optional {
                        format!("[{}] ({})", arg.name, arg.arg_type)
                    } else {
                        format!("<{}> ({})", arg.name, arg.arg_type)
                    };
                }
            }
        }

        completions
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

    fn hint(&self, line: &str, pos: usize, ctx: &Context<'_>) -> Option<Self::Hint> {
        self.hinter.hint(line, pos, ctx)
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
        self.highlighter.highlight_hint(hint)
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

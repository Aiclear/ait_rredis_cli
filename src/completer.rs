use rustyline::completion::{Completer, Pair};
use rustyline::highlight::{Highlighter, MatchingBracketHighlighter};
use rustyline::hint::{Hinter, HistoryHinter};
use rustyline::validate::{Validator, ValidationResult, ValidationContext};
use rustyline::{Context, Helper};
use std::borrow::Cow::{self, Borrowed, Owned};
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use crate::command_docs::CommandDocs;

pub struct RedisCompleter {
    commands: Arc<Mutex<HashSet<String>>>,
    command_docs: Arc<Mutex<CommandDocs>>,
    hinter: HistoryHinter,
    highlighter: MatchingBracketHighlighter,
}

impl Clone for RedisCompleter {
    fn clone(&self) -> Self {
        RedisCompleter {
            commands: Arc::clone(&self.commands),
            command_docs: Arc::clone(&self.command_docs),
            hinter: HistoryHinter::new(),
            highlighter: MatchingBracketHighlighter::new(),
        }
    }
}

impl Default for RedisCompleter {
    fn default() -> Self {
        Self::new()
    }
}

impl RedisCompleter {
    pub fn new() -> Self {
        Self::with_command_docs(CommandDocs::new())
    }
    
    pub fn with_command_docs(command_docs: CommandDocs) -> Self {
        let commands = command_docs.get_all_commands().into_iter().collect();
        
        RedisCompleter {
            commands: Arc::new(Mutex::new(commands)),
            command_docs: Arc::new(Mutex::new(command_docs)),
            hinter: HistoryHinter::new(),
            highlighter: MatchingBracketHighlighter::new(),
        }
    }
    
    /// Update command documentation from Redis
    pub fn update_command_docs(&self, command_docs: CommandDocs) {
        let commands = command_docs.get_all_commands().into_iter().collect();
        *self.commands.lock().unwrap() = commands;
        *self.command_docs.lock().unwrap() = command_docs;
    }

    pub fn get_command_hint(&self, line: &str) -> Option<String> {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return None;
        }

        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        if let Some(cmd) = parts.first() {
            let command_docs = self.command_docs.lock().unwrap();
            let hint = command_docs.get_command_hint(cmd);
            if !hint.starts_with("No documentation") {
                return Some(hint);
            }
        }
        None
    }

    pub fn get_completions(&self, prefix: &str) -> Vec<String> {
        let upper_prefix = prefix.to_uppercase();
        let commands = self.commands.lock().unwrap();
        commands
            .iter()
            .filter(|cmd| cmd.starts_with(&upper_prefix))
            .cloned()
            .collect()
    }
}

impl Completer for RedisCompleter {
    type Candidate = Pair;

    fn complete(&self, line: &str, pos: usize, _ctx: &Context<'_>) -> rustyline::Result<(usize, Vec<Pair>)> {
        let (start, word) = extract_word(line, pos);
        
        let completions = self.get_completions(word);
        let pairs = completions
            .into_iter()
            .map(|cmd| Pair {
                display: cmd.clone(),
                replacement: cmd.to_lowercase(),
            })
            .collect();

        Ok((start, pairs))
    }
}

impl Hinter for RedisCompleter {
    type Hint = String;

    fn hint(&self, line: &str, pos: usize, ctx: &Context<'_>) -> Option<Self::Hint> {
        if let Some(history_hint) = self.hinter.hint(line, pos, ctx) {
            return Some(history_hint);
        }

        self.get_command_hint(line)
    }
}

impl Highlighter for RedisCompleter {
    fn highlight_prompt<'b, 's: 'b, 'p: 'b>(
        &'s self,
        prompt: &'p str,
        default: bool,
    ) -> Cow<'b, str> {
        if default {
            Owned(format!("\x1b[1;32m{}\x1b[0m", prompt))
        } else {
            Borrowed(prompt)
        }
    }

    fn highlight_hint<'h>(&self, hint: &'h str) -> Cow<'h, str> {
        Owned(format!("\x1b[90m{}\x1b[0m", hint))
    }

    fn highlight<'l>(&self, line: &'l str, pos: usize) -> Cow<'l, str> {
        self.highlighter.highlight(line, pos)
    }

    fn highlight_char(&self, line: &str, pos: usize, forced: bool) -> bool {
        self.highlighter.highlight_char(line, pos, forced)
    }
}

impl Validator for RedisCompleter {
    fn validate(&self, _ctx: &mut ValidationContext) -> rustyline::Result<ValidationResult> {
        Ok(ValidationResult::Valid(None))
    }
}

impl Helper for RedisCompleter {}

fn extract_word(line: &str, pos: usize) -> (usize, &str) {
    let start = line[0..pos]
        .rfind(|c: char| c.is_whitespace())
        .map_or(0, |i| i + 1);
    (start, &line[start..pos])
}

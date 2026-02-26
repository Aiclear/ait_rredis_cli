use std::{
    borrow::Cow::{self, Owned},
    sync::{Arc, Mutex},
};

use rustyline::{
    completion::Completer,
    error::ReadlineError,
    highlight::Highlighter,
    hint::Hinter,
    validate::Validator,
    Helper, Context,
};

use crate::{
    command_helpers::CommandDocs,
    redis_client::RedisClient,
};

pub struct RedisHelper {
    client: Arc<Mutex<RedisClient>>,
    docs: Arc<Mutex<CommandDocs>>,
    commands: Vec<String>,
}

impl RedisHelper {
    pub fn new(client: Arc<Mutex<RedisClient>>, docs: Arc<Mutex<CommandDocs>>) -> Self {
        let cmds = docs.lock().unwrap().get_commands_list(&mut client.lock().unwrap());
        Self {
            client,
            docs,
            commands: cmds,
        }
    }

    fn extract_first_word(&self, line: &str) -> Option<String> {
        line.trim()
            .split_whitespace()
            .next()
            .map(|s| s.to_uppercase())
    }
}

impl Completer for RedisHelper {
    type Candidate = String;

    fn complete(
        &self,
        line: &str,
        _pos: usize,
        _ctx: &Context<'_>,
    ) -> Result<(usize, Vec<Self::Candidate>), ReadlineError> {
        let line_upper = line.trim().to_uppercase();
        
        let matches: Vec<String> = self
            .commands
            .iter()
            .filter(|cmd| cmd.starts_with(&line_upper))
            .cloned()
            .collect();

        if !matches.is_empty() {
            Ok((0, matches))
        } else {
            Ok((line.len(), Vec::new()))
        }
    }
}

impl Hinter for RedisHelper {
    type Hint = String;

    fn hint(&self, line: &str, _pos: usize, _ctx: &Context<'_>) -> Option<Self::Hint> {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return None;
        }

        let first_word = self.extract_first_word(trimmed)?;
        
        if first_word.starts_with('_') {
            if first_word == "_HISTORY" {
                return Some(" - Browse command history".to_string());
            }
            if first_word == "_MONITOR" {
                return Some(" - Open TUI monitoring dashboard".to_string());
            }
            return None;
        }

        let mut client = self.client.lock().ok()?;
        let mut docs = self.docs.lock().ok()?;
        
        let doc_clone = docs.get_doc(&mut client, &first_word).cloned();
        drop(docs);
        
        if let Some(doc) = doc_clone {
            let hint = CommandDocs::format_hint_static(&doc);
            if !hint.is_empty() {
                return Some(format!(" - {}", hint));
            }
        }

        None
    }
}

impl Highlighter for RedisHelper {
    fn highlight_hint<'h>(&self, hint: &'h str) -> Cow<'h, str> {
        Owned(format!("\x1b[90m{}\x1b[0m", hint))
    }
}

impl Validator for RedisHelper {}

impl Helper for RedisHelper {}

pub fn create_editor(
    client: Arc<Mutex<RedisClient>>,
    docs: Arc<Mutex<CommandDocs>>,
) -> anyhow::Result<rustyline::Editor<RedisHelper, rustyline::history::DefaultHistory>> {
    let helper = RedisHelper::new(client, docs);
    let config = rustyline::Config::builder()
        .completion_type(rustyline::CompletionType::Circular)
        .build();
    
    let mut editor = rustyline::Editor::with_config(config)?;
    editor.set_helper(Some(helper));
    
    Ok(editor)
}

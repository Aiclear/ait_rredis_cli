use std::borrow::Cow;
use std::cell::RefCell;
use std::env;
use std::rc::Rc;

use rustyline::completion::{Completer, FilenameCompleter, Pair};
use rustyline::error::ReadlineError;
use rustyline::highlight::{Highlighter, MatchingBracketHighlighter};
use rustyline::hint::{Hinter, HistoryHinter};
use rustyline::history::DefaultHistory;
use rustyline::validate::{MatchingBracketValidator, Validator};
use rustyline::{CompletionType, Config, Context, EditMode, Editor};

use crate::{
    completer::CommandCompleter,
    history::CommandHistory,
    monitor::run_monitor,
    redis_client::{RedisAddress, RedisClient},
    redis_type::{Hello, RespType},
};

mod byte_buffer;
mod completer;
mod history;
mod monitor;
mod redis_client;
mod redis_type;

struct RedisHelper {
    completer: FilenameCompleter,
    _highlighter: MatchingBracketHighlighter,
    _validator: MatchingBracketValidator,
    hinter: HistoryHinter,
    cmd_completer: Rc<RefCell<CommandCompleter>>,
    redis_client: Rc<RefCell<RedisClient>>,
}

impl Completer for RedisHelper {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &Context<'_>,
    ) -> rustyline::Result<(usize, Vec<Pair>)> {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return Ok((0, Vec::new()));
        }

        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        if parts.is_empty() {
            return Ok((0, Vec::new()));
        }

        let mut cmd_completer = self.cmd_completer.borrow_mut();
        let mut redis_client = self.redis_client.borrow_mut();
        
        let suggestions = cmd_completer.get_suggestions(&mut redis_client, trimmed);
        
        if !suggestions.is_empty() {
            let hint = &suggestions[0];
            if !hint.starts_with('=') && !hint.starts_with('\n') {
                let pair = Pair {
                    display: hint.clone(),
                    replacement: String::new(),
                };
                return Ok((pos, vec![pair]));
            }
        }

        self.completer.complete(line, pos, _ctx)
    }
}

impl Hinter for RedisHelper {
    type Hint = String;

    fn hint(&self, line: &str, pos: usize, ctx: &Context<'_>) -> Option<String> {
        if pos < line.len() {
            return self.hinter.hint(line, pos, ctx);
        }
        
        let trimmed = line.trim();
        
        if trimmed.is_empty() {
            return self.hinter.hint(line, pos, ctx);
        }

        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        if parts.is_empty() {
            return self.hinter.hint(line, pos, ctx);
        }

        let mut cmd_completer = self.cmd_completer.borrow_mut();
        let mut redis_client = self.redis_client.borrow_mut();
        
        let suggestions = cmd_completer.get_suggestions(&mut redis_client, trimmed);
        
        if !suggestions.is_empty() {
            let hint = &suggestions[0];
            if !hint.starts_with('=') && !hint.starts_with('\n') {
                return Some(format!("  {}", hint));
            }
        }

        self.hinter.hint(line, pos, ctx)
    }
}

impl Highlighter for RedisHelper {
    fn highlight<'l>(&self, line: &'l str, _pos: usize) -> Cow<'l, str> {
        Cow::Borrowed(line)
    }

    fn highlight_prompt<'b, 's: 'b, 'p: 'b>(
        &'s self,
        prompt: &'p str,
        _default: bool,
    ) -> Cow<'b, str> {
        Cow::Borrowed(prompt)
    }

    fn highlight_hint<'h>(&self, hint: &'h str) -> Cow<'h, str> {
        Cow::Owned(format!("\x1b[36m{}\x1b[0m", hint))
    }
}

impl Validator for RedisHelper {
    fn validate(
        &self,
        _ctx: &mut rustyline::validate::ValidationContext,
    ) -> rustyline::Result<rustyline::validate::ValidationResult> {
        Ok(rustyline::validate::ValidationResult::Valid(None))
    }

    fn validate_while_typing(&self) -> bool {
        false
    }
}

impl rustyline::Helper for RedisHelper {}

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = env::args().collect();

    let redis_address = if args.len() == 2 {
        RedisAddress::new(&args[1], 6379, Hello::no_auth())
    } else if args.len() == 3 {
        RedisAddress::new(&args[1], args[2].parse()?, Hello::no_auth())
    } else if args.len() == 4 {
        RedisAddress::new(
            &args[1],
            args[2].parse()?,
            Hello::with_password("default", &args[3]),
        )
    } else {
        println!("./rredis-cli.exe usage: ./rredis-cli.exe host [port [password]]");
        return Ok(());
    };

    let redis_client = RedisClient::connect(redis_address)?;
    let redis_client = Rc::new(RefCell::new(redis_client));

    let mut history = CommandHistory::new();
    let cmd_completer = Rc::new(RefCell::new(CommandCompleter::new()));

    let config = Config::builder()
        .history_ignore_space(true)
        .completion_type(CompletionType::List)
        .edit_mode(EditMode::Emacs)
        .build();

    let helper = RedisHelper {
        completer: FilenameCompleter::new(),
        _highlighter: MatchingBracketHighlighter::new(),
        _validator: MatchingBracketValidator::new(),
        hinter: HistoryHinter::new(),
        cmd_completer: cmd_completer.clone(),
        redis_client: redis_client.clone(),
    };

    let mut rl: Editor<RedisHelper, DefaultHistory> = Editor::with_config(config)?;
    rl.set_helper(Some(helper));

    loop {
        let readline = rl.readline("> ");

        match readline {
            std::result::Result::Ok(line) => {
                let trimmed = line.trim();

                if trimmed.is_empty() {
                    continue;
                }

                if trimmed == "quit" {
                    break;
                }

                if CommandHistory::is_history_command(trimmed) {
                    history.display();
                    history.add(trimmed);
                    continue;
                }

                if completer::is_monitor_command(trimmed) {
                    history.add(trimmed);
                    {
                        let mut rc = redis_client.borrow_mut();
                        if let Err(e) = run_monitor(&mut rc) {
                            eprintln!("Monitor error: {}", e);
                        }
                    }
                    println!("Exited monitor mode.");
                    continue;
                }

                history.add(trimmed);
                let _ = rl.add_history_entry(trimmed);

                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                if !parts.is_empty() {
                    let mut cc = cmd_completer.borrow_mut();
                    let mut rc = redis_client.borrow_mut();
                    let suggestions = cc.get_suggestions(&mut rc, trimmed);
                    for suggestion in &suggestions {
                        if suggestion.starts_with('\n') || suggestion.starts_with('=') {
                            println!("{}", suggestion);
                        }
                    }
                }

                let resp_type = RespType::create_from_command_line(trimmed);
                {
                    let mut rc = redis_client.borrow_mut();
                    rc.write_command(resp_type)?;

                    let response = rc.read_resp()?;
                    println!("{response}");
                }
            }
            Err(ReadlineError::Interrupted) => {
                println!("^C");
                continue;
            }
            Err(ReadlineError::Eof) => {
                println!("^D");
                break;
            }
            Err(err) => {
                eprintln!("Error: {:?}", err);
                break;
            }
        }
    }

    Ok(())
}

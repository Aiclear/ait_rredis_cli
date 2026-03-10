use std::{
    env::{self},
    path::PathBuf,
    sync::Arc,
};
use rustyline::{
    completion::{Completer, Pair},
    highlight::Highlighter,
    hint::Hinter,
    validate::Validator,
    Context, Editor, Helper,
};
use rustyline::config::Configurer;
use tokio;
use tokio::sync::Mutex;

use crate::{
    command_docs::CommandDocs,
    history::CommandHistory,
    redis_client::{RedisAddress, RedisClient},
    redis_type::{Hello, RespType},
    tui_monitor::run_monitor,
};

mod byte_buffer;
mod command_docs;
mod history;
mod redis_client;
mod redis_type;
mod tui_monitor;

struct RedisHelper {
    command_docs: Arc<Mutex<CommandDocs>>,
}

impl Completer for RedisHelper {
    type Candidate = Pair;

    fn complete(&self, _line: &str, _pos: usize, _ctx: &Context<'_>) -> rustyline::Result<(usize, Vec<Self::Candidate>)> {
        rustyline::Result::Ok((0, Vec::new()))
    }
}

impl Highlighter for RedisHelper {}

impl Validator for RedisHelper {}

impl Helper for RedisHelper {}

impl Hinter for RedisHelper {
    type Hint = String;

    fn hint(&self, line: &str, pos: usize, _ctx: &Context<'_>) -> Option<Self::Hint> {
        if pos == 0 || line.is_empty() {
            return None;
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            return None;
        }

        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        if parts.is_empty() {
            return None;
        }

        let command = parts[0].to_lowercase();
        
        let handle = tokio::runtime::Handle::current();
        let hint = handle.block_on(async {
            let docs = self.command_docs.lock().await;
            docs.get_hint(&command)
        });
        
        if !hint.contains("not found") {
            Some(format!("\n{}", hint))
        } else {
            None
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
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
        return Result::Ok(());
    };

    let mut redis_client = RedisClient::connect(redis_address.clone())?;

    let history_path = PathBuf::from(".rredis_cli_history");
    let mut command_history = CommandHistory::with_file(1000, history_path)?;

    let command_docs = Arc::new(Mutex::new(CommandDocs::new()));

    let helper = RedisHelper {
        command_docs: command_docs.clone(),
    };

    let mut rl = Editor::new()?;
    rl.set_helper(Some(helper));
    rl.set_auto_add_history(true);
    let _ = rl.set_history_ignore_dups(true);

    println!("Redis CLI with history, hints, and monitor support");
    println!("Type '_history' to view command history");
    println!("Type '_monitor' to view TUI monitor (press 'q' to exit)");
    println!("Type 'quit' to exit");

    loop {
        let readline = rl.readline("> ");
        match readline {
            Result::Ok(line) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }

                command_history.add(trimmed.to_string());

                match trimmed {
                    "quit" => break,
                    "_history" => {
                        println!("Command History:");
                        for (i, cmd) in command_history.get_all().iter().enumerate() {
                            println!("{}: {}", i + 1, cmd);
                        }
                    }
                    "_monitor" => {
                        let mut monitor_client = RedisClient::connect(redis_address.clone())?;
                        run_monitor(&mut monitor_client)?;
                    }
                    command => {
                        let parts: Vec<&str> = command.split_whitespace().collect();
                        if !parts.is_empty() {
                            let cmd = parts[0].to_lowercase();
                            let mut docs = command_docs.lock().await;
                            docs.get_command_doc(&cmd).await;
                        }

                        let resp_type = RespType::create_from_command_line(command);
                        redis_client.write_command(resp_type)?;
                        let response = redis_client.read_resp()?;
                        println!("{response}");
                    }
                }
            }
            Result::Err(rustyline::error::ReadlineError::Interrupted) => {
                println!("CTRL-C");
                break;
            }
            Result::Err(rustyline::error::ReadlineError::Eof) => {
                println!("CTRL-D");
                break;
            }
            Result::Err(err) => {
                println!("Error: {:?}", err);
                break;
            }
        }
    }

    Result::Ok(())
}

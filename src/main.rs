use std::{
    env,
    sync::{Arc, Mutex},
};

use rustyline::{
    completion::Completer,
    highlight::Highlighter,
    hint::Hinter,
    validate::Validator,
    Context, Helper, Result as RustylineResult,
};

use crate::{
    command_history::CommandHistory,
    command_hints::CommandHints,
    monitor::MonitorApp,
    redis_client::{RedisAddress, RedisClient},
    redis_type::{Hello, RespType},
};

mod byte_buffer;
mod command_history;
mod command_hints;
mod monitor;
mod redis_client;
mod redis_type;

struct RedisHelper {
    hints_cache: Arc<Mutex<CommandHints>>,
    command_names: Arc<Mutex<Vec<String>>>,
}

impl RedisHelper {
    fn new() -> Self {
        Self {
            hints_cache: Arc::new(Mutex::new(CommandHints::new())),
            command_names: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn load_command_names(&self, client: &mut RedisClient) {
        if let Ok(names) = Self::fetch_command_names(client) {
            if let Ok(mut cmd_names) = self.command_names.lock() {
                *cmd_names = names;
            }
        }
    }

    fn fetch_command_names(client: &mut RedisClient) -> anyhow::Result<Vec<String>> {
        let resp_type = RespType::create_from_command_line("COMMAND LIST");
        client.write_command(resp_type)?;
        let response = client.read_resp()?;

        let mut names = Vec::new();
        if let RespType::Arrays(arr) = response {
            for item in arr.value {
                if let RespType::BulkStrings(bs) = item {
                    names.push(bs.value.to_uppercase());
                }
            }
        }
        Ok(names)
    }
}

impl Helper for RedisHelper {}

impl Completer for RedisHelper {
    type Candidate = String;

    fn complete(
        &self,
        line: &str,
        _pos: usize,
        _ctx: &Context<'_>,
    ) -> RustylineResult<(usize, Vec<Self::Candidate>)> {
        let line = line.trim().to_uppercase();
        let candidates: Vec<String> = self
            .command_names
            .lock()
            .map(|names| {
                names
                    .iter()
                    .filter(|name| name.starts_with(&line))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();
        
        std::result::Result::Ok((0usize, candidates))
    }
}

impl Hinter for RedisHelper {
    type Hint = String;

    fn hint(&self, line: &str, _pos: usize, _ctx: &Context<'_>) -> Option<Self::Hint> {
        let line = line.trim();
        if line.is_empty() {
            return None;
        }

        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.is_empty() {
            return None;
        }

        let cmd = parts[0].to_uppercase();
        let arg_index = parts.len().saturating_sub(1);

        // 只缓存中获取提示，不创建新连接
        let hints = self.hints_cache.lock().ok()?;
        hints.get_cached_hint(&cmd, arg_index)
    }
}

impl Highlighter for RedisHelper {
    fn highlight_hint<'h>(&self, hint: &'h str) -> std::borrow::Cow<'h, str> {
        std::borrow::Cow::Owned(format!("\x1b[90m{}\x1b[0m", hint))
    }
}

impl Validator for RedisHelper {}

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

    let mut redis_client = RedisClient::connect(redis_address.clone())?;

    let mut rl = rustyline::Editor::<RedisHelper, rustyline::history::DefaultHistory>::new()?;
    let helper = RedisHelper::new();
    
    // 预加载命令列表
    helper.load_command_names(&mut redis_client);
    
    rl.set_helper(Some(helper));

    println!("Connected to Redis. Type 'quit' to exit, '_history' to view command history.");
    println!("Type '_monitor' to enter monitoring mode.");

    let mut command_history = CommandHistory::new();
    let mut command_hints = CommandHints::new();

    loop {
        let readline = rl.readline("> ");
        match readline {
            std::result::Result::Ok(input) => {
                let input = input.trim();
                
                if input.is_empty() {
                    continue;
                }

                rl.add_history_entry(input)?;
                command_history.add(input.to_string());

                match input {
                    "quit" | "exit" => break,
                    "_history" => {
                        command_history.display_history();
                    }
                    "_monitor" => {
                        let mut monitor = MonitorApp::new();
                        if let Err(e) = monitor.run(&redis_address) {
                            eprintln!("Monitor error: {}", e);
                        }
                        println!("Exited monitor mode.");
                    }
                    command => {
                        let parts: Vec<&str> = command.split_whitespace().collect();
                        if !parts.is_empty() {
                            // 执行时获取详细提示并缓存
                            if let Some(hint) = command_hints.get_hint(&mut redis_client, parts[0]) {
                                println!("\x1b[90m{}\x1b[0m", hint);
                            }
                            
                            // 同时更新到Hinter的缓存
                            if let Some(helper) = rl.helper() {
                                if let Ok(mut hints_cache) = helper.hints_cache.lock() {
                                    hints_cache.update_from(&command_hints);
                                }
                            }
                        }

                        let resp_type = RespType::create_from_command_line(command);
                        
                        if let Err(e) = redis_client.write_command(resp_type) {
                            eprintln!("Error sending command: {}", e);
                            continue;
                        }

                        match redis_client.read_resp() {
                            std::result::Result::Ok(response) => {
                                println!("{}", response);
                            }
                            Err(e) => {
                                eprintln!("Error reading response: {}", e);
                            }
                        }
                    }
                }
            }
            Err(rustyline::error::ReadlineError::Interrupted) => {
                println!("CTRL-C");
                break;
            }
            Err(rustyline::error::ReadlineError::Eof) => {
                println!("CTRL-D");
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

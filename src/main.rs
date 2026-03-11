use std::env;

use anyhow::Result;
use dirs::home_dir;
use rustyline::{Editor, Config};

use crate::{
    completer::RedisCompleter,
    redis_client::{RedisAddress, RedisClient},
    redis_type::{Hello, RespType},
    monitor::Monitor,
};

mod byte_buffer;
mod redis_client;
mod redis_type;
mod command_docs;
mod completer;
mod monitor;

fn main() -> Result<()> {
    // parse command line arguments
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

    // create client
    let mut redis_client = RedisClient::connect(redis_address)?;

    // create rustyline editor
    let config = Config::builder()
        .history_ignore_space(true)
        .completion_type(rustyline::CompletionType::List)
        .edit_mode(rustyline::EditMode::Emacs)
        .build();

    let completer = RedisCompleter::new();
    let mut rl = Editor::with_config(config)?;
    rl.set_helper(Some(completer));

    // load history from file
    let history_path = home_dir()
        .map(|p| p.join(".rredis_cli_history"))
        .unwrap_or_else(|| ".rredis_cli_history".into());
    
    if history_path.exists()
        && let Err(e) = rl.load_history(&history_path)
    {
        eprintln!("Warning: Could not load history: {}", e);
    }

    // command history storage
    let mut command_history: Vec<String> = Vec::new();

    println!("Welcome to rredis-cli!");
    println!("Type '_history' to view command history");
    println!("Type '_monitor' to monitor Redis server");
    println!("Type 'quit' to exit");

    // loop for user input
    loop {
        match rl.readline("> ") {
            Ok(line) => {
                let trimmed = line.trim();
                
                if trimmed.is_empty() {
                    continue;
                }

                // Add to history
                let _ = rl.add_history_entry(trimmed);
                command_history.push(trimmed.to_string());

                match trimmed {
                    "quit" | "exit" => break,
                    "_history" => {
                        println!("Command History ({} commands):", command_history.len());
                        for (i, cmd) in command_history.iter().rev().take(50).enumerate() {
                            println!("{}: {}", command_history.len() - i, cmd);
                        }
                        if command_history.len() > 50 {
                            println!("... ({} more commands)", command_history.len() - 50);
                        }
                    }
                    "_monitor" => {
                        println!("Entering monitor mode... Press 'q' or 'ESC' to exit");
                        let mut monitor = Monitor::new(redis_client);
                        redis_client = monitor.run()?;
                        println!("Exited monitor mode");
                    }
                    command => {
                        let resp_type = RespType::create_from_command_line(command);
                        // Send command to Redis server
                        if let Err(e) = redis_client.write_command(resp_type) {
                            eprintln!("Error sending command: {}", e);
                            continue;
                        }

                        // Read response from Redis server
                        match redis_client.read_resp() {
                            Ok(response) => println!("{}", response),
                            Err(e) => eprintln!("Error reading response: {}", e),
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

    // save history to file
    if let Err(e) = rl.save_history(&history_path) {
        eprintln!("Warning: Could not save history: {}", e);
    }

    Ok(())
}

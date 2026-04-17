use std::{
    env::{self},
    sync::{Arc, Mutex},
};

use rustyline::Editor;
use tokio::runtime::Runtime;

use crate::{
    command_completer::{RedisCommandCompleter, RedisHelper},
    redis_client::{RedisAddress, RedisClient},
    redis_type::{Hello, RespType},
    tui_monitor::{RedisMetrics, run_monitor},
};

mod byte_buffer;
mod command_completer;
mod redis_client;
mod redis_type;
mod tui_monitor;

fn main() -> anyhow::Result<()> {
    // Parse command line arguments
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

    // Create client
    let mut redis_client = RedisClient::connect(redis_address)?;

    // Get command docs for completion
    let command_completer = {
        let mut completer = RedisCommandCompleter::new();
        
        // Execute COMMAND DOCS to get all commands
        let command_docs = RespType::create_from_command_line("COMMAND DOCS");
        redis_client.write_command(command_docs)?;
        let response = redis_client.read_resp()?;
        
        if !response.is_err_type() {
            completer.parse_command_docs(&response);
            println!("Loaded {} commands for completion", completer.get_command_count());
        } else {
            println!("Warning: Could not load command docs for completion");
        }
        
        completer
    };

    // Create RedisHelper for rustyline
    let helper = RedisHelper::new(command_completer);

    // Create rustyline editor with history support
    let mut rl = Editor::new()?;
    rl.set_helper(Some(helper));

    // Load history from file if it exists
    let history_path = dirs::home_dir().map(|mut path| {
        path.push(".rredis-cli_history");
        path
    });

    if let Some(ref path) = history_path {
        if rl.load_history(path).is_err() {
            println!("No previous history found");
        }
    }

    // Main loop for user input
    loop {
        // Read user input with rustyline (supports completion and history)
        let readline = rl.readline("> ");
        
        match readline {
            Ok(line) => {
                let line_trimmed = line.trim();
                
                // Skip empty lines
                if line_trimmed.is_empty() {
                    continue;
                }
                
                // Add to history
                rl.add_history_entry(line_trimmed);
                
                // Process commands
                match line_trimmed {
                    "quit" | "exit" => {
                        println!("Bye!");
                        break;
                    }
                    "monitor" => {
                        // Start TUI monitor
                        println!("Starting monitor mode... Press 'q' or 'Esc' to exit");
                        
                        // Create metrics shared state
                        let metrics = Arc::new(Mutex::new(RedisMetrics::default()));
                        
                        // Spawn a task to update metrics
                        let metrics_clone = Arc::clone(&metrics);
                        let mut monitor_client = redis_client.try_clone()?;
                        
                        let rt = Runtime::new()?;
                        rt.block_on(async {
                            // Spawn metrics update task
                            let metrics_task = tokio::spawn(async move {
                                let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(1));
                                loop {
                                    interval.tick().await;
                                    
                                    // Get INFO all
                                    let info_cmd = RespType::create_from_command_line("INFO all");
                                    if monitor_client.write_command(info_cmd).is_ok() {
                                        if let std::result::Result::Ok(response) = monitor_client.read_resp() {
                                            if !response.is_err_type() {
                                                if let Some(new_metrics) = RedisMetrics::parse_info_response(&response) {
                                                    let mut m = metrics_clone.lock().unwrap();
                                                    *m = new_metrics;
                                                }
                                            }
                                        }
                                    }
                                }
                            });
                            
                            // Run TUI monitor
                            let monitor_result = run_monitor(Arc::clone(&metrics)).await;
                            
                            // Abort metrics task
                            metrics_task.abort();
                            
                            if let Err(e) = monitor_result {
                                eprintln!("Monitor error: {}", e);
                            }
                        });
                        
                        println!("Exited monitor mode");
                    }
                    command => {
                        // Check if we can provide hints for this command
                        if let Some(helper) = rl.helper() {
                            let parts: Vec<&str> = command.split_whitespace().collect();
                            if !parts.is_empty() {
                                let _cmd_info = helper.completer().get_command(parts[0]);
                                // Show command information if it's a new command
                                // This helps users understand what arguments are expected
                            }
                        }
                        
                        // Execute the command
                        let resp_type = RespType::create_from_command_line(command);
                        
                        // Send command to Redis server
                        redis_client.write_command(resp_type)?;
                        
                        // Read response from Redis server
                        let response = redis_client.read_resp()?;
                        
                        // Print response
                        println!("{response}");
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

    // Save history to file
    if let Some(ref path) = history_path {
        if rl.save_history(path).is_err() {
            eprintln!("Failed to save history");
        }
    }

    Ok(())
}

use std::{
    env::{self},
    sync::{Arc, Mutex},
};

use rustyline::Editor;
use tokio::runtime::Runtime;

use crate::{
    command_completer::{RedisCommandCompleter, RedisHelper},
    redis_client::{RedisAddress, RedisClient},
    redis_type::Hello,
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
    let redis_client = RedisClient::connect(redis_address)?;

    // Create command completer with built-in commands
    let mut command_completer = RedisCommandCompleter::new();
    
    // Wrap client in Arc<Mutex> for shared access
    let client_arc = Arc::new(Mutex::new(redis_client));
    
    // Set redis client in completer (will try to fetch command list from server)
    command_completer.set_redis_client(Arc::clone(&client_arc));

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
                let _ = rl.add_history_entry(line_trimmed);
                
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
                        let monitor_client = match client_arc.lock().unwrap().try_clone() {
                            Ok(c) => c,
                            Err(e) => {
                                eprintln!("Failed to clone connection for monitor: {}", e);
                                continue;
                            }
                        };
                        
                        let rt = Runtime::new()?;
                        rt.block_on(async {
                            // Spawn metrics update task
                            let metrics_task = tokio::spawn(async move {
                                let mut monitor_client = monitor_client;
                                let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(1));
                                loop {
                                    interval.tick().await;
                                    
                                    // Get INFO all
                                    let info_cmd = crate::redis_type::RespType::create_from_command_line("INFO all");
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
                        // Execute the command
                        let resp_type = crate::redis_type::RespType::create_from_command_line(command);
                        
                        // Get the client and send command
                        let mut client = client_arc.lock().unwrap();
                        
                        // Send command to Redis server
                        client.write_command(resp_type)?;
                        
                        // Read response from Redis server
                        let response = client.read_resp()?;
                        
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

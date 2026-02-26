use std::{
    env,
    sync::{Arc, Mutex},
};



use crate::{
    cli::create_editor,
    command_helpers::CommandDocs,
    history::{format_history, CmdHistory},
    monitor::run_monitor,
    redis_client::{RedisAddress, RedisClient},
    redis_type::{Hello, RespType},
};

mod byte_buffer;
mod cli;
mod command_helpers;
mod history;
mod monitor;
mod redis_client;
mod redis_type;

const HISTORY_FILE: &str = ".rredis_cli_history";
const MAX_HISTORY: usize = 1000;

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
        return anyhow::Ok(());
    };

    let redis_client = RedisClient::connect(redis_address)?;
    let client = Arc::new(Mutex::new(redis_client));
    let docs = Arc::new(Mutex::new(CommandDocs::new()));

    let mut cmd_history = CmdHistory::new(MAX_HISTORY);
    let _ = cmd_history.load(HISTORY_FILE);

    let mut editor = create_editor(client.clone(), docs.clone())?;
    let _ = editor.load_history(HISTORY_FILE);

    loop {
        let readline = editor.readline("> ");

        match readline {
            std::result::Result::Ok(line) => {
                let trimmed: &str = line.trim();
                
                match trimmed {
                    "" => continue,
                    "quit" | "QUIT" | "exit" | "EXIT" => break,
                    "_history" | "_HISTORY" => {
                        show_history(&cmd_history);
                    }
                    "_monitor" | "_MONITOR" => {
                        println!("Entering monitor mode...");
                        if let Err(e) = run_monitor(&mut client.lock().unwrap()) {
                            eprintln!("Monitor error: {}", e);
                        }
                        println!("Exited monitor mode.");
                    }
                    command => {
                        cmd_history.add(command);
                        let _ = editor.add_history_entry(command);
                        
                        let resp_type = RespType::create_from_command_line(command);
                        let result = {
                            let mut c = client.lock().unwrap();
                            c.write_command(resp_type)?;
                            c.read_resp()
                        };

                        match result {
                            std::result::Result::Ok(response) => println!("{response}"),
                            Err(e) => eprintln!("Error: {}", e),
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

    let _ = cmd_history.save(HISTORY_FILE);
    let _ = editor.save_history(HISTORY_FILE);

    Ok(())
}

fn show_history(history: &CmdHistory) {
    if history.len() == 0 {
        println!("No history yet.");
        return;
    }

    let pages = format_history(history, 20);
    let total = history.len();
    
    println!("=== Command History ({} entries) ===", total);
    for page in pages {
        for entry in page {
            println!("{}", entry);
        }
        println!();
    }
}

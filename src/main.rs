use std::env::{self};
use std::result::Result::Ok;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use anyhow::Result as AnyhowResult;
use rustyline::Editor;

use crate::{
    command_cache::CommandCache,
    redis_client::{RedisAddress, RedisClient},
    redis_type::Hello,
    smart_completer::SmartCompleter,
};

mod byte_buffer;
mod command_cache;
mod redis_client;
mod redis_type;
mod smart_completer;

fn main() -> AnyhowResult<()> {
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

    // 创建命令缓存
    let command_cache = Arc::new(Mutex::new(CommandCache::new()));

    // 启动后台线程来获取命令信息和更新keys
    let cache_clone = command_cache.clone();
    let host = args[1].clone();
    let port = if args.len() >= 3 {
        args[2].parse::<u16>().unwrap_or(6379)
    } else {
        6379
    };

    thread::spawn(move || {
        let mut client =
            match RedisClient::connect(RedisAddress::new(&host, port, Hello::no_auth())) {
                Ok(c) => c,
                Err(_) => {
                    eprintln!("Warning: Could not connect to Redis for command cache");
                    return;
                }
            };

        // 获取命令文档
        if let Err(e) = cache_clone.lock().unwrap().fetch_command_docs(&mut client) {
            eprintln!("Warning: Could not fetch command docs: {}", e);
        }

        loop {
            // 更新keys缓存
            let _ = cache_clone.lock().unwrap().update_keys(&mut client);

            thread::sleep(Duration::from_secs(30));
        }
    });

    // 创建智能补全器
    let completer = SmartCompleter::new(command_cache.clone());
    let mut editor = Editor::<SmartCompleter, rustyline::history::DefaultHistory>::new()?;
    editor.set_helper(Some(completer));

    println!("Redis CLI with smart completion");
    println!("Type 'help' for available commands or 'quit' to exit");
    println!("Press Tab for command completion");

    // loop for user input
    loop {
        match editor.readline("> ") {
            Ok(line) => {
                let command: &str = line.trim();
                if command.is_empty() {
                    continue;
                }

                if command == "quit" || command == "exit" {
                    break;
                }

                if command == "help" {
                    print_help();
                    editor.add_history_entry(command.to_string())?;
                    continue;
                }

                // 添加到历史记录
                editor.add_history_entry(command.to_string())?;

                // 执行命令
                match redis_client.execute_command(command) {
                    Ok(response) => {
                        println!("{}", response);
                    }
                    Err(e) => {
                        eprintln!("Error: {}", e);
                    }
                }
            }
            Err(rustyline::error::ReadlineError::Interrupted) => {
                println!("^C");
                break;
            }
            Err(rustyline::error::ReadlineError::Eof) => {
                println!("^D");
                break;
            }
            Err(err) => {
                println!("Error: {:?}", err);
                break;
            }
        }
    }

    Ok(())
}

fn print_help() {
    println!("Available commands:");
    println!("  GET <key>           - Get value of key");
    println!("  SET <key> <value>   - Set value of key");
    println!("  DEL <key>           - Delete key");
    println!("  KEYS <pattern>      - Find all keys matching pattern");
    println!("  EXISTS <key>        - Check if key exists");
    println!("  TYPE <key>          - Get type of key");
    println!("  TTL <key>           - Get time to live of key");
    println!("  EXPIRE <key> <seconds> - Set expiration on key");
    println!("  INFO [section]      - Get information and statistics about server");
    println!("  CONFIG GET <parameter> - Get configuration parameter");
    println!("  CONFIG SET <parameter> <value> - Set configuration parameter");
    println!("  PING                - Ping server");
    println!("  FLUSHDB             - Remove all keys from current database");
    println!("  FLUSHALL            - Remove all keys from all databases");
    println!("");
    println!("Hash commands:");
    println!("  HGET <key> <field>  - Get value of field in hash");
    println!("  HSET <key> <field> <value> - Set field in hash");
    println!("  HDEL <key> <field>  - Delete field from hash");
    println!("  HGETALL <key>       - Get all fields and values in hash");
    println!("");
    println!("List commands:");
    println!("  LPUSH <key> <value> - Prepend value to list");
    println!("  RPUSH <key> <value> - Append value to list");
    println!("  LPOP <key>          - Remove and get first element");
    println!("  RPOP <key>          - Remove and get last element");
    println!("  LLEN <key>          - Get length of list");
    println!("");
    println!("Set commands:");
    println!("  SADD <key> <member> - Add member to set");
    println!("  SREM <key> <member> - Remove member from set");
    println!("  SMEMBERS <key>      - Get all members in set");
    println!("  SCARD <key>         - Get number of members in set");
    println!("");
    println!("Sorted Set commands:");
    println!("  ZADD <key> <score> <member> - Add member to sorted set");
    println!("  ZREM <key> <member> - Remove member from sorted set");
    println!("  ZRANGE <key> <start> <stop> - Get range of members");
    println!("  ZCARD <key>         - Get number of members in sorted set");
    println!("");
    println!("Features:");
    println!("  - Tab completion for commands and keys");
    println!("  - Smart parameter suggestions");
    println!("  - Command history (use arrow keys)");
    println!("  - Context-aware completion");
}

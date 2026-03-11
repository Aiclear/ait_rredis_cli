use std::time::Duration;
use std::io::{Read, Write};
use rredis_cli::redis_client::{RedisClient, RedisAddress};
use rredis_cli::redis_type::{Hello, RespType};
use crossterm::terminal::{enable_raw_mode, disable_raw_mode};
use crossterm::event;

fn main() -> std::io::Result<()> {
    let addr = RedisAddress::new("127.0.0.1", 6379, Hello::no_auth());
    let mut client = RedisClient::connect(addr).map_err(|e| std::io::Error::other(e))?;
    
    println!("\n=== Test 3: Test with crossterm event polling ===");
    enable_raw_mode()?;
    println!("Raw mode enabled");
    
    for i in 0..3 {
        println!("Redis command {}:", i + 1);
        let cmd = RespType::create_from_command_line("PING");
        client.write_command(cmd).map_err(|e| std::io::Error::other(e))?;
        let resp = client.read_resp().map_err(|e| std::io::Error::other(e))?;
        println!("Response: {}", resp);
        
        // Poll for events - this might be the issue
        println!("Polling for events...");
        if event::poll(Duration::from_millis(100))? {
            let evt = event::read()?;
            println!("Event: {:?}", evt);
        }
        
        std::thread::sleep(Duration::from_millis(200));
    }
    
    disable_raw_mode()?;
    println!("Raw mode disabled");
    
    println!("\n=== Test 4: Test INFO command (larger response) ===");
    let addr = RedisAddress::new("127.0.0.1", 6379, Hello::no_auth());
    let mut client = RedisClient::connect(addr).map_err(|e| std::io::Error::other(e))?;
    
    enable_raw_mode()?;
    println!("Raw mode enabled");
    
    for i in 0..3 {
        println!("INFO command {}:", i + 1);
        let cmd = RespType::create_from_command_line("INFO ALL");
        if let Err(e) = client.write_command(cmd) {
            println!("Write error: {}", e);
            disable_raw_mode()?;
            return Err(std::io::Error::other(e));
        }
        match client.read_resp() {
            Ok(resp) => {
                let s = format!("{}", resp);
                println!("Response length: {}", s.len());
                println!("First 100 chars: {}", &s.chars().take(100).collect::<String>());
            },
            Err(e) => {
                println!("Read error: {}", e);
                disable_raw_mode()?;
                return Err(std::io::Error::other(e));
            }
        }
        
        if event::poll(Duration::from_millis(100))? {
            let _ = event::read()?;
        }
        
        std::thread::sleep(Duration::from_millis(500));
    }
    
    disable_raw_mode()?;
    println!("Raw mode disabled");
    
    println!("\n=== All tests passed! ===");
    
    Ok(())
}

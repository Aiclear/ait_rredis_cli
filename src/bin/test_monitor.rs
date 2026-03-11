use std::time::Duration;
use rredis_cli::redis_client::{RedisClient, RedisAddress};
use rredis_cli::redis_type::{Hello, RespType};
use crossterm::terminal::{enable_raw_mode, disable_raw_mode};

fn main() -> std::io::Result<()> {
    let addr = RedisAddress::new("127.0.0.1", 6379, Hello::no_auth());
    let mut client = RedisClient::connect(addr).map_err(|e| std::io::Error::other(e))?;
    
    println!("Testing connection without raw mode first...");
    for i in 0..3 {
        println!("Test {}:", i + 1);
        let cmd = RespType::create_from_command_line("INFO ALL");
        client.write_command(cmd).map_err(|e| std::io::Error::other(e))?;
        let resp = client.read_resp().map_err(|e| std::io::Error::other(e))?;
        let resp_str = format!("{}", resp);
        println!("Response length: {}", resp_str.len());
        std::thread::sleep(Duration::from_millis(500));
    }
    
    println!("\nNow testing with raw mode...");
    enable_raw_mode()?;
    println!("Raw mode enabled");
    
    for i in 0..3 {
        println!("Test {} (raw mode):", i + 1);
        let cmd = RespType::create_from_command_line("INFO ALL");
        if let Err(e) = client.write_command(cmd) {
            println!("Write error: {}", e);
            disable_raw_mode()?;
            return Err(std::io::Error::other(e));
        }
        match client.read_resp() {
            Ok(resp) => {
                let resp_str = format!("{}", resp);
                println!("Response length: {}", resp_str.len());
            }
            Err(e) => {
                println!("Read error: {}", e);
                disable_raw_mode()?;
                return Err(std::io::Error::other(e));
            }
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    
    disable_raw_mode()?;
    println!("Raw mode disabled");
    println!("All tests passed!");
    
    Ok(())
}

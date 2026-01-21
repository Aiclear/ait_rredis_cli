use std::{
    env::{self},
    io::{self, Write},
};

use anyhow::Ok;

use crate::{
    redis_client::{RedisAddress, RedisClient},
    redis_type::{Hello, RespType},
};

mod byte_buffer;
mod redis_client;
mod redis_type;

fn main() -> anyhow::Result<()> {
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

    // loop for user input
    loop {
        // Print prompt
        print!("> ");
        io::stdout().flush()?;

        // Read user input
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        // Process user input
        match input.trim() {
            "quit" => break,
            command => {
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

    Ok(())
}

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
    let redis_address = RedisAddress::new("127.0.0.1", 6379, Hello::no_auth());

    // create client
    let mut redis_client = RedisClient::connect(redis_address)?;

    // set hello world
    let resp_type = RespType::create_from_command_line("set hello world");
    // Send command to Redis server
    redis_client.write_command(resp_type)?;

    // Read response from Redis server
    let response = redis_client.read_resp()?;

    // Print response
    println!("{response}");

    Ok(())
}

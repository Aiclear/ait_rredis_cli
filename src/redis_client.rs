use std::{
    io::{self, Read, Write},
    net::TcpStream,
};

use anyhow::anyhow;

use crate::{
    byte_buffer::BytesBuffer,
    redis_type::{Hello, RespType},
};

/// default 4MB buffer size
const BUFFER_SIZE: usize = 4 * 1024 * 1024;

/// redis server address
pub struct RedisAddress {
    /// server host
    host: String,
    /// server port
    port: u16,
    /// auth client basic info
    hello: Hello,
}

impl RedisAddress {
    pub fn new(host: &str, port: u16, hello: Hello) -> Self {
        Self {
            host: host.to_string(),
            port,
            hello,
        }
    }

    pub fn address(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }

    pub fn hello(&self) -> Vec<u8> {
        self.hello.encode()
    }
}

struct XTcpStream(TcpStream);

impl XTcpStream {
    fn read(&mut self, buffer: &mut BytesBuffer) -> io::Result<()> {
        // write bytes to buffer we should add w_pos
        let count = self.0.read(buffer.as_recv_mut_slice())?;
        buffer.w_pos_forward(count);

        Ok(())
    }

    fn write(&mut self, buffer: &mut BytesBuffer) -> io::Result<()> {
        self.0.write_all(buffer.as_send_slice())?;
        self.0.flush()?;

        Ok(())
    }
}

pub struct RedisClient {
    buffer: BytesBuffer,
    xstream: XTcpStream,
}

impl RedisClient {
    pub fn connect(redis_address: RedisAddress) -> anyhow::Result<Self> {
        // connect to redis server
        let mut stream = TcpStream::connect(redis_address.address())?;

        // handshake
        stream.write(&redis_address.hello()[..])?;
        stream.flush()?;

        // check handshake resp
        let mut client = Self {
            buffer: BytesBuffer::new(BUFFER_SIZE),
            xstream: XTcpStream(stream),
        };

        let result = client.read_resp()?;
        if result.is_err_type() {
            // Print error message
            eprintln!("Error: {}", result);
            return Err(anyhow!("connect failed"));
        } else {
            // print handshake resp
            println!("Connected successfully!");
            println!("{result}");
        }

        Ok(client)
    }

    pub fn write_command(&mut self, resp_type: RespType) -> anyhow::Result<()> {
        // encode command
        resp_type.encode(&mut self.buffer);

        // flush buffer
        self.xstream.write(&mut self.buffer)?;

        Ok(())
    }

    pub fn read_resp(&mut self) -> anyhow::Result<RespType> {
        loop {
            self.xstream.read(&mut self.buffer)?;
            if let Some(resp) = RespType::decode(&mut self.buffer) {
                return Ok(resp);
            }
            // If we get here, we need more data
        }
    }
}

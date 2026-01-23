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
    fn read(&mut self, buffer: &mut BytesBuffer) -> io::Result<usize> {
        // write bytes to buffer we should add w_pos
        let count = self.0.read(buffer.as_recv_mut_slice())?;
        buffer.w_pos_forward(count);

        Ok(count)
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
        // Clear buffer before encoding command to avoid old data interference
        self.buffer.clear();
        
        // encode command
        resp_type.encode(&mut self.buffer);

        // flush buffer
        self.xstream.write(&mut self.buffer)?;

        Ok(())
    }

    pub fn read_resp(&mut self) -> anyhow::Result<RespType> {
        let mut attempt_count = 0;
        let max_attempts = 10;
        
        loop {
            // Try to decode first if there's any data in buffer
            if let Some(resp) = RespType::decode(&mut self.buffer) {
                return Ok(resp);
            }
            
            // If we've tried multiple times and still can't decode, skip the data
            attempt_count += 1;
            if attempt_count >= max_attempts {
                // Skip the error data and start fresh
                self.buffer.skip_to_end();
                attempt_count = 0;
            }
            
            // Read data into buffer
            // This will block until data is available or connection is closed
            let bytes_read = self.xstream.read(&mut self.buffer)?;
            
            // If no bytes were read, connection is closed
            if bytes_read == 0 {
                return Err(anyhow!("Connection closed by server"));
            }
        }
    }
}

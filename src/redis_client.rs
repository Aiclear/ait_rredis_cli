use std::{io::Write, net::TcpStream};

use anyhow::anyhow;

use crate::{
    byte_buffer::BytesBuffer,
    redis_type::{Hello, RespType},
};

/// default 4MB buffer size
const BUFFER_SIZE: usize = 1 * 1024 * 1024;

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

    pub fn hello_info(&self) -> &Hello {
        &self.hello
    }
}

struct XTcpStream(TcpStream);

impl XTcpStream {
    fn read(&mut self, buffer: &mut BytesBuffer) -> anyhow::Result<()> {
        // write bytes to buffer we should add w_pos
        let count = buffer.read_bytes(&mut self.0)?;
        if 0 == count {
            return Err(anyhow::anyhow!("Connection closed"));
        }

        Ok(())
    }

    fn write(&mut self, buffer: &mut BytesBuffer) -> anyhow::Result<()> {
        buffer.write_bytes(&mut self.0)?;
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

        // handshake using inline format (compatible with more Redis versions)
        let hello_cmd = redis_address.hello_info().encode_inline();
        stream.write(&hello_cmd[..])?;
        stream.flush()?;

        // check handshake resp
        let mut client = Self {
            buffer: BytesBuffer::new(BUFFER_SIZE),
            xstream: XTcpStream(stream),
        };

        let result = client.read_resp()?;
        if result.is_err_type() {
            // Print error message
            eprintln!("Error during handshake: {}", result);
            eprintln!("Trying without HELLO command...");
            
            // Fallback: try to connect without HELLO command
            return Self::connect_simple(redis_address);
        } else {
            // print handshake resp
            println!("Connected successfully!");
            println!("{result}");
        }

        Ok(client)
    }

    /// Simple connection without HELLO command (for older Redis versions)
    fn connect_simple(redis_address: RedisAddress) -> anyhow::Result<Self> {
        // connect to redis server
        let stream = TcpStream::connect(redis_address.address())?;

        let mut client = Self {
            buffer: BytesBuffer::new(BUFFER_SIZE),
            xstream: XTcpStream(stream),
        };

        // Try to authenticate if password is provided
        let hello = redis_address.hello_info();
        if hello.has_password() {
            let auth_cmd = if let Some(username) = hello.username() {
                // AUTH username password (Redis 6+)
                RespType::create_from_command_line(&format!("AUTH {} {}", username, hello.password().unwrap()))
            } else {
                // AUTH password (Redis < 6)
                RespType::create_from_command_line(&format!("AUTH {}", hello.password().unwrap()))
            };
            
            client.write_command(auth_cmd)?;
            let result = client.read_resp()?;
            if result.is_err_type() {
                eprintln!("Authentication failed: {}", result);
                return Err(anyhow!("Authentication failed"));
            }
        }

        // Send PING to test connection
        let ping_cmd = RespType::create_from_command_line("PING");
        client.write_command(ping_cmd)?;
        let result = client.read_resp()?;
        
        if result.is_err_type() {
            eprintln!("Connection test failed: {}", result);
            return Err(anyhow!("Connection failed"));
        } else {
            println!("Connected successfully!");
            println!("{}", result);
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
        // read byte from tcp stream
        self.xstream.read(&mut self.buffer)?;
        // decode response
        Ok(RespType::decode(&mut self.buffer))
    }

    pub fn try_clone(&self) -> anyhow::Result<Self> {
        // Try to clone the TCP stream
        let cloned_stream = self.xstream.0.try_clone()?;
        
        Ok(Self {
            buffer: BytesBuffer::new(BUFFER_SIZE),
            xstream: XTcpStream(cloned_stream),
        })
    }
}

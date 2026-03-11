use std::{io::Write, net::TcpStream};

use anyhow::anyhow;

use crate::{
    byte_buffer::BytesBuffer,
    redis_type::{Hello, RespType},
};

/// default 1MB buffer size
const BUFFER_SIZE: usize = 1024 * 1024;

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
    fn read(&mut self, buffer: &mut BytesBuffer) -> anyhow::Result<()> {
        let count = buffer.read_bytes(&mut self.0)?;
        if 0 == count {
            return Err(anyhow::anyhow!("Connection closed"));
        }
        Ok(())
    }

    fn write_all(&mut self, data: &[u8]) -> anyhow::Result<()> {
        self.0.write_all(data)?;
        self.0.flush()?;
        Ok(())
    }
}

pub struct RedisClient {
    read_buffer: BytesBuffer,
    write_buffer: BytesBuffer,
    xstream: XTcpStream,
}

impl RedisClient {
    pub fn connect(redis_address: RedisAddress) -> anyhow::Result<Self> {
        let mut stream = TcpStream::connect(redis_address.address())?;

        // handshake
        stream.write_all(&redis_address.hello()[..])?;
        stream.flush()?;

        let mut client = Self {
            read_buffer: BytesBuffer::new(BUFFER_SIZE),
            write_buffer: BytesBuffer::new(BUFFER_SIZE),
            xstream: XTcpStream(stream),
        };

        let result = client.read_resp()?;
        if result.is_err_type() {
            eprintln!("Error: {}", result);
            return Err(anyhow!("connect failed"));
        } else {
            println!("Connected successfully!");
            println!("{result}");
        }

        Ok(client)
    }

    pub fn write_command(&mut self, resp_type: RespType) -> anyhow::Result<()> {
        // Reset write buffer before encoding new command
        self.write_buffer.reset_buffer();
        
        // Encode command to write buffer
        resp_type.encode(&mut self.write_buffer);
        
        // Get the encoded data
        let data = self.write_buffer.get_available_data();
        
        // Write directly to stream
        self.xstream.write_all(data)?;

        Ok(())
    }

    pub fn read_resp(&mut self) -> anyhow::Result<RespType> {
        loop {
            match RespType::decode(&mut self.read_buffer) {
                Ok(resp) => return Ok(resp),
                Err(crate::redis_type::RespError::Incomplete) => {
                    self.xstream.read(&mut self.read_buffer)?;
                }
                Err(e) => return Err(anyhow::anyhow!(e)),
            }
        }
    }
    
    /// Clear any pending data in buffers - useful before transferring client ownership
    pub fn clear_buffers(&mut self) {
        self.read_buffer.reset_buffer();
        self.write_buffer.reset_buffer();
    }
}

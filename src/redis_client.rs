use std::{io::Write, net::TcpStream, time::Duration};

use anyhow::anyhow;
use socket2::{Socket, TcpKeepalive};

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
        // connect to redis server using socket2 for more control
        let addr: std::net::SocketAddr = redis_address.address().parse()?;
        let socket = Socket::new(
            socket2::Domain::for_address(addr),
            socket2::Type::STREAM,
            Some(socket2::Protocol::TCP),
        )?;
        
        socket.set_nodelay(true)?;
        socket.set_read_timeout(Some(Duration::from_secs(30)))?;
        socket.set_write_timeout(Some(Duration::from_secs(30)))?;
        
        let keepalive = TcpKeepalive::new()
            .with_time(Duration::from_secs(60))
            .with_interval(Duration::from_secs(10));
        socket.set_tcp_keepalive(&keepalive)?;
        
        socket.set_linger(Some(Duration::from_secs(0)))?;
        
        socket.connect(&addr.into())?;
        
        let mut stream: TcpStream = socket.into();
        
        stream.set_nonblocking(false)?;

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
        // read byte from tcp stream
        self.xstream.read(&mut self.buffer)?;
        // decode response
        Ok(RespType::decode(&mut self.buffer))
    }
}

use std::{
    collections::{BTreeMap, HashSet},
    fmt,
    hash::Hash,
};

use crate::byte_buffer::BytesBuffer;

/// redis resp type default terminator
const TERMINATOR: &'static [u8; 2] = b"\r\n";

/// this redis client support resp version
enum ProtoVer {
    Resp2,
    Resp3,
}

impl ProtoVer {
    pub fn newest_ver() -> Self {
        ProtoVer::Resp3
    }

    pub fn str_ver(&self) -> &'static str {
        match self {
            ProtoVer::Resp2 => "2",
            ProtoVer::Resp3 => "3",
        }
    }
}

pub struct Hello {
    username: Option<String>,
    password: Option<String>,
    client_name: String,
}

impl Hello {
    pub fn no_auth() -> Hello {
        Hello {
            username: None,
            password: None,
            client_name: "rredis_cli".to_string(),
        }
    }

    pub fn with_password(username: &str, password: &str) -> Hello {
        Hello {
            username: Some(username.to_string()),
            password: Some(password.to_string()),
            client_name: "rredis_cli".to_string(),
        }
    }

    pub fn encode(&self) -> Vec<u8> {
        // hello proto_ver [auth username password setname client_name]
        let mut hello_v = vec![];

        // hello proto_ver
        hello_v.extend_from_slice(b"HELLO ");
        hello_v.extend_from_slice(ProtoVer::newest_ver().str_ver().as_bytes());
        hello_v.push(b' ');

        // auth username password
        if self.password.is_some() {
            hello_v.extend_from_slice(b"AUTH ");
            hello_v.extend_from_slice(
                self.username
                    .as_ref()
                    .unwrap_or(&"default".to_string())
                    .as_bytes(),
            );
            hello_v.push(b' ');
            hello_v.extend_from_slice(self.password.as_ref().unwrap().as_bytes());
            hello_v.push(b' ');
        }

        // setname
        hello_v.extend_from_slice(b"SETNAME ");
        hello_v.extend_from_slice(self.client_name.as_bytes());
        hello_v.push(b' ');

        // terminator
        hello_v.extend_from_slice(b"\r\n");

        hello_v
    }
}

/// redis type struct
pub enum RespType {
    SimpleStrings(SimpleString),
    BulkStrings(BulkString),
    Integers(Integer),
    Booleans(Boolean),
    Nulls(Null),
    Maps(Map),
    Sets(Set),
    Arrays(Array),
    SimpleErrors(SimpleError),
    BulkErrors(BulkError),
}

impl RespType {
    pub fn decode(buff: &mut BytesBuffer) -> anyhow::Result<RespType> {
        if !buff.has_remaining() {
            return Err(anyhow::anyhow!("Insufficient data"));
        }
        
        let byte = buff.get_u8();
        match byte {
            SimpleString::PLUS => Ok(RespType::SimpleStrings(SimpleString::decode(buff)?)),
            BulkString::DOLLAR => Ok(RespType::BulkStrings(BulkString::decode(buff)?)),
            Integer::COLON => Ok(RespType::Integers(Integer::decode(buff)?)),
            Boolean::OCTOTHORPE => Ok(RespType::Booleans(Boolean::decode(buff)?)),
            Null::UNDERSCORE => Ok(RespType::Nulls(Null::decode(buff)?)),
            Map::PERCENT => Ok(RespType::Maps(Map::decode(buff)?)),
            Set::TIDLE => Ok(RespType::Sets(Set::decode(buff)?)),
            Array::STAR => Ok(RespType::Arrays(Array::decode(buff)?)),
            SimpleError::MINUS => Ok(RespType::SimpleErrors(SimpleError::decode(buff)?)),
            BulkError::EXCLAMATION => Ok(RespType::BulkErrors(BulkError::decode(buff)?)),

            _ => Err(anyhow::anyhow!("Invalid resp type")),
        }
    }

    /// build a RespType from command line input
    /// like `set hello world` => Array([SimpleString("set"), BulkString("hello"), BulkString("world")])
    pub fn create_from_command_line(value: &str) -> RespType {
        let arrays: Vec<RespType> = value
            .split(" ")
            .map(|t| RespType::BulkStrings(BulkString::new(t.to_string())))
            .collect();

        RespType::Arrays(Array::new(arrays))
    }

    pub fn encode(&self, buff: &mut BytesBuffer) {
        match self {
            RespType::Arrays(array) => array.encode(buff),
            RespType::BulkStrings(bulk_string) => bulk_string.encode(buff),

            _ => panic!("Invalid resp type"),
        }
    }

    pub fn is_err_type(&self) -> bool {
        match self {
            RespType::SimpleErrors(_) | RespType::BulkErrors(_) => true,
            _ => false,
        }
    }
}

impl fmt::Display for RespType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RespType::SimpleStrings(ss) => write!(f, "{}", ss.value),
            RespType::BulkStrings(bs) => write!(f, "{}", bs.value),
            RespType::Integers(i) => write!(f, "{}", i.value),
            RespType::Booleans(b) => write!(f, "{}", b.value),
            RespType::Nulls(_) => write!(f, "{}", "nil"),
            RespType::Maps(m) => {
                if m.map.is_empty() {
                    return write!(f, "{}", "{}");
                }

                m.map.iter().for_each(|(key, value)| {
                    writeln!(f, "{}: {}", key.1, value).unwrap();
                });
                fmt::Result::Ok(())
            }
            RespType::Sets(s) => {
                if s.value.is_empty() {
                    return write!(f, "{}", "#{}");
                }

                s.value.iter().for_each(|e| write!(f, "{}", e.1).unwrap());
                fmt::Result::Ok(())
            }
            RespType::Arrays(a) => {
                if a.value.is_empty() {
                    return write!(f, "{}", "[]");
                }

                a.value.iter().for_each(|e| write!(f, "{}", e).unwrap());
                fmt::Result::Ok(())
            }
            RespType::SimpleErrors(se) => write!(f, "{}", se.value),
            RespType::BulkErrors(be) => write!(f, "{}", be.value),
        }
    }
}

pub struct SimpleString {
    value: String,
}

impl SimpleString {
    const PLUS: u8 = b'+';

    pub fn decode(buff: &mut BytesBuffer) -> anyhow::Result<SimpleString> {
        let string_bytes = buff.get_slice_until(TERMINATOR);
        if string_bytes.is_empty() {
            buff.reset();
            return Err(anyhow::anyhow!("Insufficient data"));
        }
        
        Ok(SimpleString {
            value: String::from_utf8_lossy(string_bytes).to_string(),
        })
    }
}

/// $<length>\r\n<data>\r\n
pub struct BulkString {
    value: String,
}

impl BulkString {
    const DOLLAR: u8 = b'$';

    pub fn new(value: String) -> BulkString {
        BulkString { value }
    }

    pub fn decode(buff: &mut BytesBuffer) -> anyhow::Result<BulkString> {
        // length
        let length_slice = buff.get_slice_until(TERMINATOR);
        if length_slice.is_empty() {
            buff.reset();
            return Err(anyhow::anyhow!("Insufficient data"));
        }
        
        let bytes_length = String::from_utf8_lossy(length_slice)
            .parse::<usize>()
            .map_err(|_| anyhow::anyhow!("Invalid length"))?;

        // read data
        if buff.remaining() < bytes_length + 2 {
            buff.reset();
            return Err(anyhow::anyhow!("Insufficient data"));
        }
        
        let value = String::from_utf8_lossy(buff.get_slice(bytes_length)).to_string();

        // terminator
        buff.get_u8();
        buff.get_u8();

        Ok(BulkString { value })
    }

    pub fn encode(&self, buff: &mut BytesBuffer) {
        buff.put_u8(BulkString::DOLLAR);
        buff.put_u8_slice(self.value.len().to_string().as_bytes());
        buff.put_u8_slice(&TERMINATOR[..]);
        buff.put_u8_slice(self.value.as_bytes());
        buff.put_u8_slice(&TERMINATOR[..]);
    }
}

pub struct Integer {
    value: isize,
}

impl Integer {
    const COLON: u8 = b':';

    pub fn decode(buff: &mut BytesBuffer) -> anyhow::Result<Integer> {
        let digits = buff.get_slice_until(TERMINATOR);
        if digits.is_empty() {
            buff.reset();
            return Err(anyhow::anyhow!("Insufficient data"));
        }
        
        let value = String::from_utf8_lossy(digits)
            .parse::<isize>()
            .map_err(|_| anyhow::anyhow!("Invalid integer"))?;
        
        Ok(Integer { value })
    }
}

pub struct Boolean {
    value: bool,
}

impl Boolean {
    const OCTOTHORPE: u8 = b'#';

    pub fn decode(buff: &mut BytesBuffer) -> anyhow::Result<Boolean> {
        if buff.remaining() < 3 {
            buff.reset();
            return Err(anyhow::anyhow!("Insufficient data"));
        }
        
        let b_byte = buff.get_u8();

        // terminal
        buff.get_u8();
        buff.get_u8();

        let value = if b't' == b_byte { true } else { false };
        Ok(Boolean { value })
    }
}

pub struct Null;

impl Null {
    const UNDERSCORE: u8 = b'_';

    pub fn decode(buff: &mut BytesBuffer) -> anyhow::Result<Null> {
        if buff.remaining() < 2 {
            buff.reset();
            return Err(anyhow::anyhow!("Insufficient data"));
        }
        
        // terminal
        buff.get_u8();
        buff.get_u8();

        Ok(Null)
    }
}

pub struct OrderKey(usize, RespType);

impl PartialOrd for OrderKey {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.0.partial_cmp(&other.0)
    }
}

impl Ord for OrderKey {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.partial_cmp(&other.0).unwrap()
    }
}

impl PartialEq for OrderKey {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Eq for OrderKey {}

impl Hash for OrderKey {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

pub struct Map {
    map: BTreeMap<OrderKey, RespType>,
}

impl Map {
    const PERCENT: u8 = b'%';

    pub fn decode(buff: &mut BytesBuffer) -> anyhow::Result<Map> {
        // length number of elements
        let noe_slice = buff.get_slice_until(TERMINATOR);
        if noe_slice.is_empty() {
            buff.reset();
            return Err(anyhow::anyhow!("Insufficient data"));
        }
        
        let noe = String::from_utf8_lossy(noe_slice)
            .parse::<usize>()
            .map_err(|_| anyhow::anyhow!("Invalid map size"))?;

        let mut map = BTreeMap::new();
        // read terminal
        for i in 0..noe {
            let key = RespType::decode(buff)?;
            let value = RespType::decode(buff)?;

            map.insert(OrderKey(i, key), value);
        }

        Ok(Map { map })
    }
}

pub struct Set {
    value: HashSet<OrderKey>,
}

impl Set {
    const TIDLE: u8 = b'~';

    pub fn decode(buff: &mut BytesBuffer) -> anyhow::Result<Set> {
        // number of elements
        let noe_slice = buff.get_slice_until(TERMINATOR);
        if noe_slice.is_empty() {
            buff.reset();
            return Err(anyhow::anyhow!("Insufficient data"));
        }
        
        let noe = String::from_utf8_lossy(noe_slice)
            .parse::<usize>()
            .map_err(|_| anyhow::anyhow!("Invalid set size"))?;

        let mut value = HashSet::with_capacity(noe);
        // read elements
        for i in 0..noe {
            value.insert(OrderKey(i, RespType::decode(buff)?));
        }

        Ok(Set { value })
    }
}

pub struct Array {
    value: Vec<RespType>,
}

impl Array {
    const STAR: u8 = b'*';

    pub fn new(value: Vec<RespType>) -> Array {
        Array { value }
    }

    pub fn decode(buff: &mut BytesBuffer) -> anyhow::Result<Array> {
        // number of elements
        let noe_slice = buff.get_slice_until(TERMINATOR);
        if noe_slice.is_empty() {
            buff.reset();
            return Err(anyhow::anyhow!("Insufficient data"));
        }
        
        let noe = String::from_utf8_lossy(noe_slice)
            .parse::<usize>()
            .map_err(|_| anyhow::anyhow!("Invalid array size"))?;

        let mut value = Vec::with_capacity(noe);
        // read terminal
        for _ in 0..noe {
            value.push(RespType::decode(buff)?);
        }

        Ok(Array { value })
    }

    pub fn encode(&self, buff: &mut BytesBuffer) {
        buff.put_u8(Array::STAR);

        buff.put_u8_slice(self.value.len().to_string().as_bytes());
        buff.put_u8_slice(&TERMINATOR[..]);
        for item in &self.value {
            item.encode(buff);
        }
    }
}

pub struct SimpleError {
    value: String,
}

impl SimpleError {
    const MINUS: u8 = b'-';

    pub fn decode(buff: &mut BytesBuffer) -> anyhow::Result<SimpleError> {
        let value_slice = buff.get_slice_until(TERMINATOR);
        if value_slice.is_empty() {
            buff.reset();
            return Err(anyhow::anyhow!("Insufficient data"));
        }
        
        let value = String::from_utf8_lossy(value_slice).to_string();
        Ok(SimpleError { value })
    }
}

pub struct BulkError {
    value: String,
}

impl BulkError {
    const EXCLAMATION: u8 = b'!';

    pub fn decode(buff: &mut BytesBuffer) -> anyhow::Result<BulkError> {
        // length
        let length_slice = buff.get_slice_until(TERMINATOR);
        if length_slice.is_empty() {
            buff.reset();
            return Err(anyhow::anyhow!("Insufficient data"));
        }
        
        let bytes_length = String::from_utf8_lossy(length_slice)
            .parse::<usize>()
            .map_err(|_| anyhow::anyhow!("Invalid length"))?;

        // read data
        if buff.remaining() < bytes_length + 2 {
            buff.reset();
            return Err(anyhow::anyhow!("Insufficient data"));
        }
        
        let value = String::from_utf8_lossy(buff.get_slice(bytes_length)).to_string();

        // terminator
        buff.get_u8();
        buff.get_u8();

        Ok(BulkError { value })
    }
}

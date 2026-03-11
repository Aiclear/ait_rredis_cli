use std::{
    collections::{BTreeMap, HashSet},
    fmt,
    hash::Hash,
};

use num_bigint::BigInt;
use thiserror::Error;

use crate::byte_buffer::{BytesBuffer, BufferError};

/// redis resp type default terminator
const TERMINATOR: &[u8; 2] = b"\r\n";

#[derive(Error, Debug)]
pub enum RespError {
    #[error("Incomplete data")]
    Incomplete,
    #[error("Invalid RESP format: {0}")]
    InvalidFormat(String),
    #[error("Parse error: {0}")]
    ParseError(String),
}

impl From<BufferError> for RespError {
    fn from(err: BufferError) -> Self {
        match err {
            BufferError::Incomplete => RespError::Incomplete,
            BufferError::Invalid(msg) => RespError::InvalidFormat(msg),
        }
    }
}

/// this redis client support resp version
enum ProtoVer {
    #[allow(dead_code)]
    Resp2,
    Resp3,
}

impl ProtoVer {
    pub fn newest_ver() -> Self {
        ProtoVer::Resp3
    }

    pub fn str_ver(&self) -> &str {
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
        if let Some(password) = &self.password {
            hello_v.extend_from_slice(b"AUTH ");
            hello_v.extend_from_slice(
                self.username
                    .as_ref()
                    .unwrap_or(&"default".to_string())
                    .as_bytes(),
            );
            hello_v.push(b' ');
            hello_v.extend_from_slice(password.as_bytes());
            hello_v.push(b' ');
        }

        // setname
        hello_v.extend_from_slice(b"SETNAME ");
        hello_v.extend_from_slice(self.client_name.as_bytes());

        // terminator
        hello_v.extend_from_slice(b"\r\n");

        hello_v
    }
}

/// redis type struct
#[derive(Debug, Clone)]
pub enum RespType {
    SimpleStrings(SimpleString),
    BulkStrings(BulkString),
    Integers(Integer),
    Booleans(Boolean),
    Doubles(Double),
    BigNumbers(BigNumber),
    Nulls(Null),
    Maps(Map),
    Sets(Set),
    Arrays(Array),
    SimpleErrors(SimpleError),
    BulkErrors(BulkError),
    // local define resp type, no send to server
    Unknown,
}

impl RespType {
    pub fn decode(buff: &mut BytesBuffer) -> Result<RespType, RespError> {
        if !buff.has_remaining() {
            return Err(RespError::Incomplete);
        }
        
        let byte = buff.get_u8();
        match byte {
            SimpleString::PLUS => Ok(RespType::SimpleStrings(SimpleString::decode(buff)?)),
            BulkString::DOLLAR => Ok(RespType::BulkStrings(BulkString::decode(buff)?)),
            Integer::COLON => Ok(RespType::Integers(Integer::decode(buff)?)),
            Boolean::OCTOTHORPE => Ok(RespType::Booleans(Boolean::decode(buff)?)),
            Double::COMMA => Ok(RespType::Doubles(Double::decode(buff)?)),
            BigNumber::LEFT_PARENTHESIS => Ok(RespType::BigNumbers(BigNumber::decode(buff)?)),
            Null::UNDERSCORE => Ok(RespType::Nulls(Null::decode(buff)?)),
            Map::PERCENT => Ok(RespType::Maps(Map::decode(buff)?)),
            Set::TIDLE => Ok(RespType::Sets(Set::decode(buff)?)),
            Array::STAR => Ok(RespType::Arrays(Array::decode(buff)?)),
            SimpleError::MINUS => Ok(RespType::SimpleErrors(SimpleError::decode(buff)?)),
            BulkError::EXCLAMATION => Ok(RespType::BulkErrors(BulkError::decode(buff)?)),
            _ => Ok(Self::Unknown),
        }
    }

    /// build a RespType from command line input
    /// like `set hello world` => Array([SimpleString("set"), BulkString("hello"), BulkString("world")])
    pub fn create_from_command_line(value: &str) -> RespType {
        let arrays: Vec<RespType> = value
            .split_whitespace()
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
        matches!(self, RespType::SimpleErrors(_) | RespType::BulkErrors(_))
    }
    
    
    
    /// Helper method to extract array elements for COMMAND command response
    pub fn as_array(&self) -> Option<&[RespType]> {
        match self {
            RespType::Arrays(arr) => Some(&arr.value),
            _ => None,
        }
    }
    
    /// Helper method to extract bulk string value
    pub fn as_bulk_string(&self) -> Option<&str> {
        match self {
            RespType::BulkStrings(bs) => Some(&bs.value),
            _ => None,
        }
    }
    
    /// Helper method to extract simple string value
    pub fn as_simple_string(&self) -> Option<&str> {
        match self {
            RespType::SimpleStrings(ss) => Some(&ss.value),
            _ => None,
        }
    }
    
    /// Helper method to extract integer value
    pub fn as_integer(&self) -> Option<isize> {
        match self {
            RespType::Integers(i) => Some(i.value),
            _ => None,
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
            RespType::Doubles(d) => write!(f, "{}", d.value),
            RespType::BigNumbers(bn) => write!(f, "{}", bn.value),
            RespType::Nulls(_) => write!(f, "nil"),
            RespType::Maps(m) => {
                if m.map.is_empty() {
                    return write!(f, "{{}}");
                }

                for (key, value) in &m.map {
                    writeln!(f, "{}: {}", key.1, value)?;
                }
                Ok(())
            }
            RespType::Sets(s) => {
                if s.value.is_empty() {
                    return write!(f, "#{{}}");
                }

                for e in &s.value {
                    write!(f, "{} ", e.1)?;
                }
                Ok(())
            }
            RespType::Arrays(a) => {
                if a.value.is_empty() {
                    return write!(f, "[]");
                }

                for (i, e) in a.value.iter().enumerate() {
                    writeln!(f, "{}) {}", i + 1, e)?;
                }
                Ok(())
            }
            RespType::SimpleErrors(se) => write!(f, "(error) {}", se.value),
            RespType::BulkErrors(be) => write!(f, "(error) {}", be.value),
            RespType::Unknown => write!(f, "Unknown Response"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SimpleString {
    value: String,
}

impl SimpleString {
    const PLUS: u8 = b'+';

    pub fn decode(buff: &mut BytesBuffer) -> Result<SimpleString, RespError> {
        let string_bytes = buff.get_slice_until(TERMINATOR)
            .ok_or(RespError::Incomplete)?;
        Ok(SimpleString {
            value: String::from_utf8_lossy(string_bytes).to_string(),
        })
    }
}

/// $<length>\r\n<data>\r\n
#[derive(Debug, Clone)]
pub struct BulkString {
    value: String,
}

impl BulkString {
    const DOLLAR: u8 = b'$';

    pub fn new(value: String) -> BulkString {
        BulkString { value }
    }

    pub fn decode(buff: &mut BytesBuffer) -> Result<BulkString, RespError> {
        // length
        let length_bytes = buff.get_slice_until(TERMINATOR)
            .ok_or(RespError::Incomplete)?;
        let length_str = String::from_utf8_lossy(length_bytes);
        let bytes_length = length_str.parse::<isize>()
            .map_err(|e| RespError::ParseError(format!("Invalid bulk string length: {}", e)))?;
        
        // Handle NULL bulk string ($-1\r\n)
        if bytes_length == -1 {
            return Ok(BulkString { value: "(nil)".to_string() });
        }
        
        let bytes_length = bytes_length as usize;

        // Check if we have enough data
        if buff.remaining() < bytes_length + 2 {
            return Err(RespError::Incomplete);
        }

        // read data
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

#[derive(Debug, Clone)]
pub struct Integer {
    value: isize,
}

impl Integer {
    const COLON: u8 = b':';

    pub fn decode(buff: &mut BytesBuffer) -> Result<Integer, RespError> {
        let digits_bytes = buff.get_slice_until(TERMINATOR)
            .ok_or(RespError::Incomplete)?;
        let digits = String::from_utf8_lossy(digits_bytes);
        Ok(Integer {
            value: digits.parse::<isize>()
                .map_err(|e| RespError::ParseError(format!("Invalid integer: {}", e)))?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct Boolean {
    value: bool,
}

impl Boolean {
    const OCTOTHORPE: u8 = b'#';

    pub fn decode(buff: &mut BytesBuffer) -> Result<Boolean, RespError> {
        if buff.remaining() < 3 {
            return Err(RespError::Incomplete);
        }
        
        let b_byte = buff.get_u8();

        // terminal
        buff.get_u8();
        buff.get_u8();

        let value = b_byte == b't';
        Ok(Boolean { value })
    }
}

#[derive(Debug, Clone)]
pub struct Double {
    value: f64,
}

impl Double {
    const COMMA: u8 = b',';

    pub fn decode(buff: &mut BytesBuffer) -> Result<Double, RespError> {
        let digits_bytes = buff.get_slice_until(TERMINATOR)
            .ok_or(RespError::Incomplete)?;
        let digits = String::from_utf8_lossy(digits_bytes);
        Ok(Double {
            value: digits.parse::<f64>()
                .map_err(|e| RespError::ParseError(format!("Invalid double: {}", e)))?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct BigNumber {
    value: BigInt,
}

impl BigNumber {
    const LEFT_PARENTHESIS: u8 = b'(';

    pub fn decode(buff: &mut BytesBuffer) -> Result<BigNumber, RespError> {
        let digits_bytes = buff.get_slice_until(TERMINATOR)
            .ok_or(RespError::Incomplete)?;
        let digits = String::from_utf8_lossy(digits_bytes);
        Ok(BigNumber {
            value: digits.parse::<BigInt>()
                .map_err(|e| RespError::ParseError(format!("Invalid big number: {}", e)))?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct Null;

impl Null {
    const UNDERSCORE: u8 = b'_';

    pub fn decode(buff: &mut BytesBuffer) -> Result<Null, RespError> {
        if buff.remaining() < 2 {
            return Err(RespError::Incomplete);
        }
        
        // terminal
        buff.get_u8();
        buff.get_u8();

        Ok(Null)
    }
}

#[derive(Debug, Clone)]
pub struct OrderKey(usize, RespType);

impl PartialOrd for OrderKey {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for OrderKey {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.cmp(&other.0)
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

#[derive(Debug, Clone)]
pub struct Map {
    map: BTreeMap<OrderKey, RespType>,
}

impl Map {
    const PERCENT: u8 = b'%';

    pub fn decode(buff: &mut BytesBuffer) -> Result<Map, RespError> {
        // length number of elements
        let noe_bytes = buff.get_slice_until(TERMINATOR)
            .ok_or(RespError::Incomplete)?;
        let noe_str = String::from_utf8_lossy(noe_bytes);
        let noe = noe_str.parse::<usize>()
            .map_err(|e| RespError::ParseError(format!("Invalid map length: {}", e)))?;

        let mut map = BTreeMap::new();
        for i in 0..noe {
            let key = RespType::decode(buff)?;
            let value = RespType::decode(buff)?;

            map.insert(OrderKey(i, key), value);
        }

        Ok(Map { map })
    }
}

#[derive(Debug, Clone)]
pub struct Set {
    value: HashSet<OrderKey>,
}

impl Set {
    const TIDLE: u8 = b'~';

    pub fn decode(buff: &mut BytesBuffer) -> Result<Set, RespError> {
        // number of elements
        let noe_bytes = buff.get_slice_until(TERMINATOR)
            .ok_or(RespError::Incomplete)?;
        let noe_str = String::from_utf8_lossy(noe_bytes);
        let noe = noe_str.parse::<usize>()
            .map_err(|e| RespError::ParseError(format!("Invalid set length: {}", e)))?;

        let mut value = HashSet::with_capacity(noe);
        for i in 0..noe {
            value.insert(OrderKey(i, RespType::decode(buff)?));
        }

        Ok(Set { value })
    }
}

#[derive(Debug, Clone)]
pub struct Array {
    value: Vec<RespType>,
}

impl Array {
    const STAR: u8 = b'*';

    pub fn new(value: Vec<RespType>) -> Array {
        Array { value }
    }

    pub fn decode(buff: &mut BytesBuffer) -> Result<Array, RespError> {
        // number of elements
        let noe_bytes = buff.get_slice_until(TERMINATOR)
            .ok_or(RespError::Incomplete)?;
        let noe_str = String::from_utf8_lossy(noe_bytes);
        let noe = noe_str.parse::<isize>()
            .map_err(|e| RespError::ParseError(format!("Invalid array length: {}", e)))?;
        
        // Handle NULL array (*-1\r\n)
        if noe == -1 {
            return Ok(Array { value: Vec::new() });
        }
        
        let noe = noe as usize;

        let mut value = Vec::with_capacity(noe);
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

#[derive(Debug, Clone)]
pub struct SimpleError {
    value: String,
}

impl SimpleError {
    const MINUS: u8 = b'-';

    pub fn decode(buff: &mut BytesBuffer) -> Result<SimpleError, RespError> {
        let value_bytes = buff.get_slice_until(TERMINATOR)
            .ok_or(RespError::Incomplete)?;
        Ok(SimpleError {
            value: String::from_utf8_lossy(value_bytes).to_string(),
        })
    }
}

#[derive(Debug, Clone)]
pub struct BulkError {
    value: String,
}

impl BulkError {
    const EXCLAMATION: u8 = b'!';

    pub fn decode(buff: &mut BytesBuffer) -> Result<BulkError, RespError> {
        // length
        let length_bytes = buff.get_slice_until(TERMINATOR)
            .ok_or(RespError::Incomplete)?;
        let length_str = String::from_utf8_lossy(length_bytes);
        let bytes_length = length_str.parse::<usize>()
            .map_err(|e| RespError::ParseError(format!("Invalid bulk error length: {}", e)))?;

        // Check if we have enough data
        if buff.remaining() < bytes_length + 2 {
            return Err(RespError::Incomplete);
        }

        // read data
        let value = String::from_utf8_lossy(buff.get_slice(bytes_length)).to_string();

        // terminator
        buff.get_u8();
        buff.get_u8();

        Ok(BulkError { value })
    }
}

/// Structure to hold Redis command information from COMMAND command
#[derive(Debug, Clone)]
pub struct RedisCommandInfo {
    pub name: String,
    pub arity: isize,
    pub flags: Vec<String>,
    pub first_key: isize,
    pub last_key: isize,
    pub step: isize,
    pub acl_categories: Vec<String>,
    pub tips: Vec<String>,
    pub key_specifications: Vec<RespType>,
    pub subcommands: Vec<RedisCommandInfo>,
}

impl RedisCommandInfo {
    /// Parse command info from RESP array response
    pub fn from_resp(resp: &RespType) -> Option<Self> {
        let arr = resp.as_array()?;
        if arr.len() < 6 {
            return None;
        }

        let name = arr[0].as_bulk_string()?.to_uppercase();
        let arity = arr[1].as_integer()?;
        
        let flags = if let Some(flags_arr) = arr[2].as_array() {
            flags_arr.iter()
                .filter_map(|f| f.as_bulk_string().map(|s| s.to_string()))
                .collect()
        } else {
            Vec::new()
        };
        
        let first_key = arr[3].as_integer()?;
        let last_key = arr[4].as_integer()?;
        let step = arr[5].as_integer()?;
        
        // Parse additional fields (Redis 6+)
        let mut acl_categories = Vec::new();
        let mut tips = Vec::new();
        let mut key_specifications = Vec::new();
        let mut subcommands = Vec::new();
        
        if arr.len() > 6 && let Some(acl_arr) = arr[6].as_array() {
            acl_categories = acl_arr.iter()
                .filter_map(|f| f.as_bulk_string().map(|s| s.to_string()))
                .collect();
        }
        
        if arr.len() > 7 && let Some(tips_arr) = arr[7].as_array() {
            tips = tips_arr.iter()
                .filter_map(|f| f.as_bulk_string().map(|s| s.to_string()))
                .collect();
        }
        
        if arr.len() > 8 && let Some(ks_arr) = arr[8].as_array() {
            key_specifications = ks_arr.to_vec();
        }
        
        if arr.len() > 9 && let Some(sub_arr) = arr[9].as_array() {
            subcommands = sub_arr.iter()
                .filter_map(Self::from_resp)
                .collect();
        }

        Some(RedisCommandInfo {
            name,
            arity,
            flags,
            first_key,
            last_key,
            step,
            acl_categories,
            tips,
            key_specifications,
            subcommands,
        })
    }
}

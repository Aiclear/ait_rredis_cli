use std::{
    collections::{BTreeMap, HashSet},
    fmt,
    hash::Hash,
};

use num_bigint::BigInt;

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

    pub fn has_password(&self) -> bool {
        self.password.is_some()
    }

    pub fn password(&self) -> Option<&String> {
        self.password.as_ref()
    }

    pub fn username(&self) -> Option<&String> {
        self.username.as_ref()
    }

    pub fn client_name(&self) -> Option<&String> {
        Some(&self.client_name)
    }

    /// Helper function to encode a bulk string in RESP format
    fn encode_bulk_string(s: &str) -> Vec<u8> {
        let mut result = vec![];
        result.push(b'$');
        result.extend_from_slice(s.len().to_string().as_bytes());
        result.extend_from_slice(b"\r\n");
        result.extend_from_slice(s.as_bytes());
        result.extend_from_slice(b"\r\n");
        result
    }

    /// Encode HELLO command in RESP array format
    pub fn encode(&self) -> Vec<u8> {
        // Build HELLO command in RESP array format
        // Format: *<count>\r\n$<len>\r\nHELLO\r\n$<len>\r\n3\r\n...
        
        let mut parts = vec![];
        
        // HELLO command
        parts.push(Self::encode_bulk_string("HELLO"));
        
        // Protocol version
        parts.push(Self::encode_bulk_string(ProtoVer::newest_ver().str_ver()));
        
        // AUTH username password (if password is provided)
        if self.password.is_some() {
            parts.push(Self::encode_bulk_string("AUTH"));
            parts.push(Self::encode_bulk_string(self.username.as_ref().unwrap_or(&"default".to_string())));
            parts.push(Self::encode_bulk_string(self.password.as_ref().unwrap()));
        }
        
        // SETNAME client_name
        parts.push(Self::encode_bulk_string("SETNAME"));
        parts.push(Self::encode_bulk_string(&self.client_name));
        
        // Build the array
        let mut result = vec![];
        result.push(b'*');
        result.extend_from_slice(parts.len().to_string().as_bytes());
        result.extend_from_slice(b"\r\n");
        
        for part in parts {
            result.extend_from_slice(&part);
        }
        
        result
    }

    /// Encode HELLO command in inline format (compatible with older Redis versions)
    pub fn encode_inline(&self) -> Vec<u8> {
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
    pub fn decode(buff: &mut BytesBuffer) -> RespType {
        let byte = buff.get_u8();
        match byte {
            SimpleString::PLUS => RespType::SimpleStrings(SimpleString::decode(buff)),
            BulkString::DOLLAR => RespType::BulkStrings(BulkString::decode(buff)),
            Integer::COLON => RespType::Integers(Integer::decode(buff)),
            Boolean::OCTOTHORPE => RespType::Booleans(Boolean::decode(buff)),
            Double::COMMA => RespType::Doubles(Double::decode(buff)),
            BigNumber::LEFT_PARENTHESIS => RespType::BigNumbers(BigNumber::decode(buff)),
            Null::UNDERSCORE => RespType::Nulls(Null::decode(buff)),
            Map::PERCENT => RespType::Maps(Map::decode(buff)),
            Set::TIDLE => RespType::Sets(Set::decode(buff)),
            Array::STAR => RespType::Arrays(Array::decode(buff)),
            SimpleError::MINUS => RespType::SimpleErrors(SimpleError::decode(buff)),
            BulkError::EXCLAMATION => RespType::BulkErrors(BulkError::decode(buff)),

            _ => Self::Unknown,
        }
    }

    /// build a RespType from command line input
    /// like `set hello world` => Array([BulkString("set"), BulkString("hello"), BulkString("world")])
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
        match self {
            RespType::SimpleErrors(_) | RespType::BulkErrors(_) => true,
            _ => false,
        }
    }

    pub fn as_map(&self) -> Option<&Map> {
        match self {
            RespType::Maps(map) => Some(map),
            _ => None,
        }
    }

    pub fn as_bulk_string(&self) -> Option<&BulkString> {
        match self {
            RespType::BulkStrings(bs) => Some(bs),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&Array> {
        match self {
            RespType::Arrays(arr) => Some(arr),
            _ => None,
        }
    }

    pub fn as_boolean(&self) -> Option<&Boolean> {
        match self {
            RespType::Booleans(b) => Some(b),
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

                a.value.iter().for_each(|e| writeln!(f, "{}", e).unwrap());
                fmt::Result::Ok(())
            }
            RespType::SimpleErrors(se) => write!(f, "{}", se.value),
            RespType::BulkErrors(be) => write!(f, "{}", be.value),
            RespType::Unknown => write!(f, "Unknown Response"),
        }
    }
}

pub struct SimpleString {
    value: String,
}

impl SimpleString {
    const PLUS: u8 = b'+';

    pub fn decode(buff: &mut BytesBuffer) -> SimpleString {
        let string_bytes = buff.get_slice_until(TERMINATOR);
        SimpleString {
            value: String::from_utf8_lossy(string_bytes).to_string(),
        }
    }

    pub fn value(&self) -> &str {
        &self.value
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

    pub fn decode(buff: &mut BytesBuffer) -> BulkString {
        // length
        let bytes_length = String::from_utf8_lossy(buff.get_slice_until(TERMINATOR))
            .parse::<usize>()
            .unwrap();

        // read data
        let value = String::from_utf8_lossy(buff.get_slice(bytes_length)).to_string();

        // terminator
        buff.get_u8();
        buff.get_u8();

        BulkString { value }
    }

    pub fn encode(&self, buff: &mut BytesBuffer) {
        buff.put_u8(BulkString::DOLLAR);
        buff.put_u8_slice(self.value.len().to_string().as_bytes());
        buff.put_u8_slice(&TERMINATOR[..]);
        buff.put_u8_slice(self.value.as_bytes());
        buff.put_u8_slice(&TERMINATOR[..]);
    }

    pub fn value(&self) -> &str {
        &self.value
    }
}

pub struct Integer {
    value: isize,
}

impl Integer {
    const COLON: u8 = b':';

    pub fn decode(buff: &mut BytesBuffer) -> Integer {
        let digits = String::from_utf8_lossy(buff.get_slice_until(TERMINATOR));
        Integer {
            value: digits.parse::<isize>().unwrap(),
        }
    }
}

pub struct Boolean {
    value: bool,
}

impl Boolean {
    const OCTOTHORPE: u8 = b'#';

    pub fn decode(buff: &mut BytesBuffer) -> Boolean {
        let b_byte = buff.get_u8();

        // terminal
        buff.get_u8();
        buff.get_u8();

        let value = if b't' == b_byte { true } else { false };
        Boolean { value }
    }

    pub fn value(&self) -> bool {
        self.value
    }
}

pub struct Double {
    value: f64,
}

impl Double {
    const COMMA: u8 = b',';

    pub fn decode(buff: &mut BytesBuffer) -> Double {
        let digits = String::from_utf8_lossy(buff.get_slice_until(TERMINATOR));
        Double {
            value: digits.parse::<f64>().unwrap(),
        }
    }
}

pub struct BigNumber {
    value: BigInt,
}

impl BigNumber {
    const LEFT_PARENTHESIS: u8 = b'(';

    pub fn decode(buff: &mut BytesBuffer) -> BigNumber {
        let digits = String::from_utf8_lossy(buff.get_slice_until(TERMINATOR));
        BigNumber {
            value: digits.parse::<BigInt>().unwrap(),
        }
    }
}

pub struct Null;

impl Null {
    const UNDERSCORE: u8 = b'_';

    pub fn decode(buff: &mut BytesBuffer) -> Null {
        // terminal
        buff.get_u8();
        buff.get_u8();

        Null
    }
}

pub struct OrderKey(usize, RespType);

impl OrderKey {
    pub fn value(&self) -> &RespType {
        &self.1
    }

    pub fn index(&self) -> usize {
        self.0
    }
}

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

    pub fn decode(buff: &mut BytesBuffer) -> Map {
        // length number of elements
        let noe = String::from_utf8_lossy(buff.get_slice_until(TERMINATOR))
            .parse::<usize>()
            .unwrap();

        let mut map = BTreeMap::new();
        // read terminal
        for i in 0..noe {
            let key = RespType::decode(buff);
            let value = RespType::decode(buff);

            map.insert(OrderKey(i, key), value);
        }

        Map { map }
    }

    pub fn iter(&self) -> impl Iterator<Item = (&OrderKey, &RespType)> {
        self.map.iter()
    }

    pub fn get(&self, key: &OrderKey) -> Option<&RespType> {
        self.map.get(key)
    }
}

pub struct Set {
    value: HashSet<OrderKey>,
}

impl Set {
    const TIDLE: u8 = b'~';

    pub fn decode(buff: &mut BytesBuffer) -> Set {
        // number of elements
        let noe = String::from_utf8_lossy(buff.get_slice_until(TERMINATOR))
            .parse::<usize>()
            .unwrap();

        let mut value = HashSet::with_capacity(noe);
        // read elements
        for i in 0..noe {
            value.insert(OrderKey(i, RespType::decode(buff)));
        }

        Set { value }
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

    pub fn decode(buff: &mut BytesBuffer) -> Array {
        // number of elements
        let noe = String::from_utf8_lossy(buff.get_slice_until(TERMINATOR))
            .parse::<usize>()
            .unwrap();

        let mut value = Vec::with_capacity(noe);
        // read terminal
        for _ in 0..noe {
            value.push(RespType::decode(buff));
        }

        Array { value }
    }

    pub fn encode(&self, buff: &mut BytesBuffer) {
        buff.put_u8(Array::STAR);

        buff.put_u8_slice(self.value.len().to_string().as_bytes());
        buff.put_u8_slice(&TERMINATOR[..]);
        for item in &self.value {
            item.encode(buff);
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &RespType> {
        self.value.iter()
    }

    pub fn len(&self) -> usize {
        self.value.len()
    }

    pub fn is_empty(&self) -> bool {
        self.value.is_empty()
    }
}

pub struct SimpleError {
    value: String,
}

impl SimpleError {
    const MINUS: u8 = b'-';

    pub fn decode(buff: &mut BytesBuffer) -> SimpleError {
        let value = String::from_utf8_lossy(buff.get_slice_until(TERMINATOR)).to_string();
        SimpleError { value }
    }
}

pub struct BulkError {
    value: String,
}

impl BulkError {
    const EXCLAMATION: u8 = b'!';

    pub fn decode(buff: &mut BytesBuffer) -> BulkError {
        // length
        let bytes_length = String::from_utf8_lossy(buff.get_slice_until(TERMINATOR))
            .parse::<usize>()
            .unwrap();

        // read data
        let value = String::from_utf8_lossy(buff.get_slice(bytes_length)).to_string();

        // terminator
        buff.get_u8();
        buff.get_u8();

        BulkError { value }
    }
}

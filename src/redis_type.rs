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
    pub fn decode(buff: &mut BytesBuffer) -> Option<RespType> {
        if !buff.has_remaining() {
            return None;
        }
        
        let byte = buff.get_u8();
        match byte {
            SimpleString::PLUS => {
                if let Some(simple_string) = SimpleString::decode(buff) {
                    Some(RespType::SimpleStrings(simple_string))
                } else {
                    None
                }
            }
            BulkString::DOLLAR => {
                if let Some(bulk_string) = BulkString::decode(buff) {
                    Some(RespType::BulkStrings(bulk_string))
                } else {
                    None
                }
            }
            Integer::COLON => {
                if let Some(integer) = Integer::decode(buff) {
                    Some(RespType::Integers(integer))
                } else {
                    None
                }
            }
            Boolean::OCTOTHORPE => {
                if let Some(boolean) = Boolean::decode(buff) {
                    Some(RespType::Booleans(boolean))
                } else {
                    None
                }
            }
            Null::UNDERSCORE => {
                if let Some(null) = Null::decode(buff) {
                    Some(RespType::Nulls(null))
                } else {
                    None
                }
            }
            Map::PERCENT => {
                if let Some(map) = Map::decode(buff) {
                    Some(RespType::Maps(map))
                } else {
                    None
                }
            }
            Set::TIDLE => {
                if let Some(set) = Set::decode(buff) {
                    Some(RespType::Sets(set))
                } else {
                    None
                }
            }
            Array::STAR => {
                if let Some(array) = Array::decode(buff) {
                    Some(RespType::Arrays(array))
                } else {
                    None
                }
            }
            SimpleError::MINUS => {
                if let Some(simple_error) = SimpleError::decode(buff) {
                    Some(RespType::SimpleErrors(simple_error))
                } else {
                    None
                }
            }
            BulkError::EXCLAMATION => {
                if let Some(bulk_error) = BulkError::decode(buff) {
                    Some(RespType::BulkErrors(bulk_error))
                } else {
                    None
                }
            }
            _ => {
                // Skip unknown type and continue
                None
            }
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
            _ => {
                // For other types, we don't need to encode them in this context
            }
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

    pub fn decode(buff: &mut BytesBuffer) -> Option<SimpleString> {
        if let Some(string_bytes) = buff.get_slice_until(TERMINATOR) {
            Some(SimpleString {
                value: String::from_utf8_lossy(string_bytes).to_string(),
            })
        } else {
            None
        }
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

    pub fn decode(buff: &mut BytesBuffer) -> Option<BulkString> {
        // length
        if let Some(length_bytes) = buff.get_slice_until(TERMINATOR) {
            if let Ok(bytes_length) = String::from_utf8_lossy(length_bytes).parse::<usize>() {
                // Check if enough data
                if buff.has_remaining_at_least(bytes_length + 2) {
                    // read data
                    let value = String::from_utf8_lossy(buff.get_slice(bytes_length)).to_string();

                    // terminator
                    buff.get_u8();
                    buff.get_u8();

                    Some(BulkString { value })
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        }
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

    pub fn decode(buff: &mut BytesBuffer) -> Option<Integer> {
        if let Some(digits_bytes) = buff.get_slice_until(TERMINATOR) {
            if let Ok(value) = String::from_utf8_lossy(digits_bytes).parse::<isize>() {
                Some(Integer { value })
            } else {
                None
            }
        } else {
            None
        }
    }
}

pub struct Boolean {
    value: bool,
}

impl Boolean {
    const OCTOTHORPE: u8 = b'#';

    pub fn decode(buff: &mut BytesBuffer) -> Option<Boolean> {
        if buff.has_remaining_at_least(3) {
            let b_byte = buff.get_u8();

            // terminal
            buff.get_u8();
            buff.get_u8();

            let value = if b't' == b_byte { true } else { false };
            Some(Boolean { value })
        } else {
            None
        }
    }
}

pub struct Null;

impl Null {
    const UNDERSCORE: u8 = b'_';

    pub fn decode(buff: &mut BytesBuffer) -> Option<Null> {
        if buff.has_remaining_at_least(2) {
            // terminal
            buff.get_u8();
            buff.get_u8();

            Some(Null)
        } else {
            None
        }
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

    pub fn decode(buff: &mut BytesBuffer) -> Option<Map> {
        // length number of elements
        if let Some(noe_bytes) = buff.get_slice_until(TERMINATOR) {
            if let Ok(noe) = String::from_utf8_lossy(noe_bytes).parse::<usize>() {
                let mut map = BTreeMap::new();
                let mut all_decoded = true;
                
                // read elements
                for i in 0..noe {
                    if let Some(key) = RespType::decode(buff) {
                        if let Some(value) = RespType::decode(buff) {
                            map.insert(OrderKey(i, key), value);
                        } else {
                            all_decoded = false;
                            break;
                        }
                    } else {
                        all_decoded = false;
                        break;
                    }
                }

                if all_decoded {
                    Some(Map { map })
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        }
    }
}

pub struct Set {
    value: HashSet<OrderKey>,
}

impl Set {
    const TIDLE: u8 = b'~';

    pub fn decode(buff: &mut BytesBuffer) -> Option<Set> {
        // number of elements
        if let Some(noe_bytes) = buff.get_slice_until(TERMINATOR) {
            if let Ok(noe) = String::from_utf8_lossy(noe_bytes).parse::<usize>() {
                let mut value = HashSet::with_capacity(noe);
                let mut all_decoded = true;
                
                // read elements
                for i in 0..noe {
                    if let Some(element) = RespType::decode(buff) {
                        value.insert(OrderKey(i, element));
                    } else {
                        all_decoded = false;
                        break;
                    }
                }

                if all_decoded {
                    Some(Set { value })
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        }
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

    pub fn decode(buff: &mut BytesBuffer) -> Option<Array> {
        // number of elements
        if let Some(noe_bytes) = buff.get_slice_until(TERMINATOR) {
            if let Ok(noe) = String::from_utf8_lossy(noe_bytes).parse::<usize>() {
                let mut value = Vec::with_capacity(noe);
                let mut all_decoded = true;
                
                // read elements
                for _ in 0..noe {
                    if let Some(element) = RespType::decode(buff) {
                        value.push(element);
                    } else {
                        all_decoded = false;
                        break;
                    }
                }

                if all_decoded {
                    Some(Array { value })
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        }
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

    pub fn decode(buff: &mut BytesBuffer) -> Option<SimpleError> {
        if let Some(value_bytes) = buff.get_slice_until(TERMINATOR) {
            Some(SimpleError {
                value: String::from_utf8_lossy(value_bytes).to_string(),
            })
        } else {
            None
        }
    }
}

pub struct BulkError {
    value: String,
}

impl BulkError {
    const EXCLAMATION: u8 = b'!';

    pub fn decode(buff: &mut BytesBuffer) -> Option<BulkError> {
        // length
        if let Some(length_bytes) = buff.get_slice_until(TERMINATOR) {
            if let Ok(bytes_length) = String::from_utf8_lossy(length_bytes).parse::<usize>() {
                // Check if enough data
                if buff.has_remaining_at_least(bytes_length + 2) {
                    // read data
                    let value = String::from_utf8_lossy(buff.get_slice(bytes_length)).to_string();

                    // terminator
                    buff.get_u8();
                    buff.get_u8();

                    Some(BulkError { value })
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        }
    }
}

#[derive(Debug)]
pub struct Cbor<'a> {
    major: u8,
    additional_info: u8,
    argument: usize,

    start: usize,
    head_byte_count: usize,
    tail_byte_count: usize,

    body_bytes: &'a [u8],
    children: Vec<Cbor<'a>>,
}

#[derive(Debug)]
pub enum CborType {
    Uint,
    Nint,
    Bstr,
    Tstr,
    Array,
    Map,
    Tag,
    Bool,
    Null,
    Undefined,
    Float,
    Simple,
}

#[derive(Debug)]
pub enum ParseError {
    TooManyBytes(usize),               // number of bytes
    TooFewBytes(usize, usize),         // start offset, number of bytes
    ReservedAdditionalInfo(usize, u8), // start offset, additional info
    IllegalIndefiniteLength(usize),    // start offset
    BreakSymbol(usize),                // start offset
}

impl<'a> Cbor<'a> {
    pub fn from_bytes(bytes: &'a [u8]) -> Result<Self, ParseError> {
        let result = Cbor::cbor_from_bytes(bytes, 0)?;

        match bytes.len() - result.size() {
            0 => Ok(result),
            trailing_bytes => Err(ParseError::TooManyBytes(trailing_bytes)),
        }
    }

    pub fn size(&self) -> usize {
        self.head_byte_count
            + self.body_bytes.len()
            + self.children.iter().fold(0, |acc, c| acc + c.size())
            + self.tail_byte_count
    }

    pub fn get_type(&self) -> CborType {
        use CborType::*;

        match self.major {
            0 => Uint,
            1 => Nint,
            2 => Bstr,
            3 => Tstr,
            4 => Array,
            5 => Map,
            6 => Tag,
            7 => match self.additional_info {
                0..=23 => Simple,
                20 | 21 => Bool,
                22 => Null,
                23 => Undefined,
                24 => Simple,
                25..=27 => Float,
                e => panic!("Uncaught malformed major type 7 encoding, info: {}", e),
            },
            e => panic!("Uncaught malformed major type, major: {}", e),
        }
    }

    /*
    pub fn semantic(&self) -> (String, &Vec<Cbor<'a>>) {
        let description = match (self.major, self.argument) {
            (0, value) => format!("{}", value),
            (1, value) => format!("-{}", value + 1),
            (2, _) => format!("<<{:?}>>", self.body_bytes),
            (3, _) => String::from_utf8_lossy(self.body_bytes).into_owned(),
            (4, _) => format!("[{}]", self.children.len()),
            (5, _) => format!("{{{}}}", self.children.len()),
            (6, value) => format!("({})", value),
            (7, 24..=27) => String::from("Float"),
            (7, 20) => String::from("false"),
            (7, 21) => String::from("true"),
            (7, 22) => String::from("null"),
            (7, 23) => String::from("undefined"),
            (7, 31) => String::from("Break"),
            _ => String::from("Unknown"),
        };

        (description, &self.children)
    }
    */

    /*
    fn describe(&self, indent_level: usize) -> Vec<String> {
        let head = match self.major {
            0 => format!("{}", self.argument),
            1 => format!("-{}", self.argument + 1),
            2 => format!("<<{:?}>>", self.body_bytes),
            3 => String::from_utf8_lossy(self.body_bytes).into_owned(),
            4 => String::from("[\n"),
            5 => String::from("{\n"),
            6 => format!("{}(\n", self.argument),
            7 => match self.additional_info {
                0..=19 => format!("s{}", self.additional_info),
                20 => String::from("false"),
                21 => String::from("true"),
                22 => String::from("null"),
                23 => String::from("undefined"),
                24 => format!("s{}", self.argument),
                25 => format!("{}", parse_float(self.argument, 2)),
                26 => format!("{}", parse_float(self.argument, 4)),
                27 => format!("{}", parse_float(self.argument, 8)),
                31 => String::from("break"),
                e => panic!("Uncaught malformed major type 7 encoding, info: {}", e),
            },
            e => panic!("Uncaught malformed major type, major: {}", e),
        };

        let tail = match self.major {
            4 => String::from("\n]"),
            5 => String::from("\n}"),
            6 => String::from("\n)"),
            _ => String::from("\n"),
        };

        let mut pieces = Vec::new();

        pieces.push(String::from("    ").repeat(indent_level));
        pieces.push(head);

        for child in &self.children {
            pieces.extend(child.describe(indent_level + 1));
        }

        pieces.push(String::from("    ").repeat(indent_level));
        pieces.push(tail);

        pieces
    }
    */

    fn cbor_from_bytes(bytes: &'a [u8], start: usize) -> Result<Self, ParseError> {
        if bytes.first().is_none() {
            return Err(ParseError::TooFewBytes(start, 1));
        }

        let byte = bytes.first().unwrap();

        let major = byte >> 5;
        let additional_info = byte & 0x1f;

        let argument_byte_count = match additional_info {
            0..=23 => 0,
            24 => 1,
            25 => 2,
            26 => 4,
            27 => 8,
            31 => 0,
            _ => return Err(ParseError::ReservedAdditionalInfo(start, additional_info)),
        };

        let indefinite_length = additional_info == 31;

        if indefinite_length {
            match major {
                0 | 1 | 6 => return Err(ParseError::IllegalIndefiniteLength(start)),
                7 => return Err(ParseError::BreakSymbol(start)),
                _ => (),
            }
        }

        let argument = if argument_byte_count == 0 {
            additional_info as usize
        } else {
            match parse_big_endian(&bytes[1..], argument_byte_count) {
                Ok(value) => value,
                Err(bytes_missing) => return Err(ParseError::TooFewBytes(start, bytes_missing)),
            }
        };

        let head_byte_count = 1 + argument_byte_count;

        let (body_byte_count, child_count) = match major {
            2 | 3 => (argument, 0),
            4 => (0, argument),
            5 => (0, 2 * argument),
            6 => (0, 1),
            _ => (0, 0),
        };

        if body_byte_count > (&bytes[head_byte_count..]).len() {
            let bytes_missing = body_byte_count - (&bytes[head_byte_count..]).len();
            return Err(ParseError::TooFewBytes(start, bytes_missing));
        }
        let body_bytes = &bytes[head_byte_count..head_byte_count + body_byte_count];

        let mut children = Vec::with_capacity(child_count);
        let mut child_offset = head_byte_count + body_byte_count;

        if indefinite_length {
            loop {
                match Cbor::cbor_from_bytes(&bytes[child_offset..], start + child_offset) {
                    Ok(child) => {
                        child_offset += child.size();
                        children.push(child);
                    }
                    Err(ParseError::BreakSymbol(_)) => break,
                    error => return error,
                }
            }
        } else {
            for _ in 0..child_count {
                let child = Cbor::cbor_from_bytes(&bytes[child_offset..], start + child_offset)?;
                child_offset += child.size();
                children.push(child);
            }
        }

        let tail_byte_count = if indefinite_length { 1 } else { 0 };

        Ok(Self {
            major,
            additional_info,
            argument,

            start,
            head_byte_count,
            tail_byte_count,

            body_bytes,
            children,
        })
    }
}

/*
impl<'a> std::fmt::Display for Cbor<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.describe(0).join(""))
    }
}
*/

fn parse_big_endian(bytes: &[u8], size: usize) -> Result<usize, usize> {
    if size > bytes.len() {
        return Err(size - bytes.len());
    }

    let mut result = 0;
    for byte in &bytes[0..size] {
        result <<= 8;
        result |= *byte as usize;
    }

    Ok(result)
}

/*
fn parse_float(bits: usize, byte_count: usize) -> f64 {
    match byte_count {
        2 => 0.0,
        4 => 0.0,
        8 => f64::from_bits(bits as u64),
        e => panic!("Parse float with unsupported byte count: {}", e),
    }
}
*/

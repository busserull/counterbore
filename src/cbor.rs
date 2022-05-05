const INDENT_MARKER: &'static str = "    ";

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
pub enum ParseError {
    TooManyBytes(usize),               // number of bytes
    TooFewBytes(usize, usize),         // start offset, number of bytes
    ReservedAdditionalInfo(usize, u8), // start offset, additional info
    IllegalIndefiniteLength(usize),    // start offset
    BreakSymbol(usize),                // start offset
}

enum CborType {
    Uint,
    Nint,
    Bstr,
    Tstr,
    Array,
    Map,
    Tag,
    True,
    False,
    Null,
    Undefined,
    Float,
    Simple,
}

impl<'a> Cbor<'a> {
    pub fn from_bytes(bytes: &'a [u8]) -> Result<Self, ParseError> {
        let result = Cbor::cbor_from_bytes(bytes, 0)?;

        match bytes.len() - result.size() {
            0 => Ok(result),
            trailing_bytes => Err(ParseError::TooManyBytes(trailing_bytes)),
        }
    }

    fn size(&self) -> usize {
        self.head_byte_count
            + self.body_bytes.len()
            + self.children.iter().fold(0, |acc, c| acc + c.size())
            + self.tail_byte_count
    }

    fn get_type(&self) -> CborType {
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
                0..=19 => Simple,
                20 => False,
                21 => True,
                22 => Null,
                23 => Undefined,
                24 => Simple,
                25..=27 => Float,
                e => panic!("Uncaught malformed major type 7 encoding, info: {}", e),
            },
            e => panic!("Uncaught malformed major type, major: {}", e),
        }
    }

    fn format(&self, indent_level: usize) -> Vec<String> {
        use CborType::*;
        let cbor_type = self.get_type();

        let indent = INDENT_MARKER.to_string().repeat(indent_level);

        let head = match cbor_type {
            Uint => format!("{}", self.argument),
            Nint => format!("-{}", self.argument + 1),
            Bstr => "<<".to_string(),
            Tstr => format!("\"{}", String::from_utf8_lossy(self.body_bytes)),
            Array => "[\n".to_string(),
            Map => "{\n".to_string(),
            Tag => format!("{}(\n", self.argument),
            Simple => format!("simple {}", self.argument),
            True => "true".to_string(),
            False => "false".to_string(),
            Null => "null".to_string(),
            Undefined => "undefined".to_string(),
            Float => String::from("Float (representation unimplemented)"),
        };

        let tail = match cbor_type {
            Bstr => format!("{}>>", indent),
            Tstr => String::from("\""),
            Array => format!("{}]", indent),
            Map => format!("{}}}", indent),
            Tag => format!("{})", indent),
            _ => String::from(""),
        };

        let mut fragments = vec![indent, head];

        match cbor_type {
            Bstr => match Cbor::from_bytes(self.body_bytes) {
                Ok(cbor) => {
                    fragments.push(" expands to:\n".to_string());
                    fragments.extend(cbor.format(indent_level + 1));
                    fragments.push("\n".to_string());
                }
                Err(_) => {
                    fragments.push("\n".to_string());
                    for chunk in self.body_bytes.chunks(16) {
                        fragments.push(INDENT_MARKER.to_string().repeat(indent_level + 1));
                        for byte in chunk {
                            fragments.push(format!("{:02x}, ", byte));
                        }
                        fragments.push("\n".to_string());
                    }
                }
            },

            Array => {
                for child in self.children.iter() {
                    fragments.extend(child.format(indent_level + 1));
                    fragments.push(",\n".to_string());
                }
            }

            Map => {
                for pair in self.children.chunks_exact(2) {
                    let key = &pair[0];
                    let value = &pair[1];

                    fragments.extend(key.format(indent_level + 1));
                    fragments.push(" => ".to_string());
                    fragments.extend(value.format(indent_level + 1).into_iter().skip(1));
                    fragments.push(",\n".to_string());
                }

                for lone_key in self.children.chunks_exact(2).remainder() {
                    fragments.extend(lone_key.format(indent_level + 1));
                    fragments.push(" => WARNING: NO ASSOCIATED VALUE,\n".to_string());
                }
            }

            Tag => {
                let child = self.children.last().expect("Tag with no child element");
                fragments.extend(child.format(indent_level + 1));
                fragments.push("\n".to_string());
            }

            _ => {
                for child in self.children.iter() {
                    fragments.extend(child.format(indent_level + 1));
                }
            }
        }

        fragments.push(tail);

        fragments
    }

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

impl<'a> std::fmt::Display for Cbor<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.format(0).join(""))
    }
}

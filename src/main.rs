mod cbor;

fn read_cbor() -> Vec<u8> {
    vec![
        0x8a, 0x01, 0x18, 0x42, 0xd9, 0x01, 0x32, 0x21, 0x2b, 0xf5, 0xf4, 0xf6, 0xa2, 0x01, 0x64,
        0x54, 0x65, 0x73, 0x74, 0x81, 0x01, 0x65, 0x4f, 0x74, 0x68, 0x65, 0x72, 0x43, 0x82, 0x12,
        0x13, 0xfb, 0x3f, 0xf0, 0xf5, 0xc2, 0x8f, 0x5c, 0x28, 0xf6,
    ]
}
/*

#[derive(Debug, PartialEq)]
enum ElementSemanticType {
    Unsigned,
    Negative,
    ByteString,
    TextString,
    Array,
    Map,
    Tag,
    True,
    False,
    Null,
    Undefined,
    Float,
}

#[derive(Debug)]
struct Element {
    semantic: ElementSemanticType,

    major: u8,
    argument: usize,

    start: usize,
    header_byte_count: usize,
    body_byte_count: usize,

    bytes: Vec<u8>,
    children: Vec<Element>,
}

impl Element {
    fn from_cbor(cbor: &[u8], start: usize) -> Result<Self, ParseError> {
        if let Some(byte) = cbor.first() {
            let major = byte >> 5;

            let argument = (byte & 0x1f) as usize;
            let argument_size = match argument {
                0..=23 => 0,
                24 => 1,
                25 => 2,
                26 => 4,
                27 => 8,
                31 => return Err(ParseError::UnsupportedIndefiniteLength),
                _ => return Err(ParseError::ReservedArgument(start)),
            };
            let argument = match argument_size {
                0 => argument,
                size => parse_big_endian(&cbor[1..], size)?,
            };

            let header_byte_count = 1 + argument_size;
            let (body_byte_count, child_count) = match major {
                2 | 3 => (argument, 0),
                4 => (0, argument),
                5 => (0, 2 * argument),
                6 => (0, 1),
                _ => (0, 0),
            };

            let bytes = parse_bytes(&cbor[header_byte_count..], body_byte_count)?;

            let mut children = Vec::with_capacity(child_count);
            let mut child_offset = header_byte_count + body_byte_count;
            for _ in 0..child_count {
                let child = Element::from_cbor(&cbor[child_offset..], start + child_offset)?;
                child_offset += child.size();
                children.push(child);
            }

            let semantic = match (major, argument) {
                (0, _) => ElementSemanticType::Unsigned,
                (1, _) => ElementSemanticType::Negative,
                (2, _) => ElementSemanticType::ByteString,
                (3, _) => ElementSemanticType::TextString,
                (4, _) => ElementSemanticType::Array,
                (5, _) => ElementSemanticType::Map,
                (6, _) => ElementSemanticType::Tag,
                (7, 20) => ElementSemanticType::False,
                (7, 21) => ElementSemanticType::True,
                (7, 22) => ElementSemanticType::Null,
                (7, 23) => ElementSemanticType::Undefined,
                (7, 31) => return Err(ParseError::UnsupportedIndefiniteLength),
                (7, _) => ElementSemanticType::Float,
                _ => return Err(ParseError::MalformedInput),
            };

            Ok(Self {
                semantic,

                major,
                argument,

                start,
                header_byte_count,
                body_byte_count,

                bytes,
                children,
            })
        } else {
            Err(ParseError::NotEnoughBytes)
        }
    }

    fn byte_strings(&self) -> Vec<&Element> {
        let initial: Vec<&Element> = match self.semantic {
            ElementSemanticType::ByteString => vec![&self],
            _ => Vec::new(),
        };

        self.children.iter().fold(initial, |mut acc, child| {
            acc.extend_from_slice(&child.byte_strings());
            acc
        })
    }

    fn size(&self) -> usize {
        self.children.iter().fold(
            self.header_byte_count + self.body_byte_count,
            |acc, child| acc + child.size(),
        )
    }

    fn put(&self, indent: usize) {
        fn nudge(indent: usize) {
            for _ in 0..indent {
                print!("    ");
            }
        }

        use ElementSemanticType::*;

        nudge(indent);

        match self.semantic {
            Unsigned => println!("{}", self.argument),
            Negative => println!("-{}", self.argument + 1),
            ByteString => println!(
                "<< {}..{} >>",
                self.bytes.first().unwrap(),
                self.bytes.last().unwrap()
            ),
            TextString => println!("\"{}\"", String::from(String::from_utf8_lossy(&self.bytes))),
            Array => println!("["),
            Map => println!("{{"),
            Tag => println!("{}(", self.argument),
            True => println!("true"),
            False => println!("false"),
            Null => println!("null"),
            Undefined => println!("undefined"),
            Float => println!("Float: {}", self.argument),
        }

        for child in &self.children {
            child.put(indent + 1);
        }

        match self.semantic {
            Array => {
                nudge(indent);
                println!("]")
            }
            Map => {
                nudge(indent);
                println!("}}")
            }
            Tag => {
                nudge(indent);
                println!(")")
            }
            _ => (),
        }
    }
}

impl std::fmt::Display for Element {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let type_line = match self.major {
            0 => format!("{}", self.argument),
            1 => format!("-{}", self.argument + 1),
            2 => format!("<<{}>>", self.argument),
            3 => String::from(String::from_utf8_lossy(&self.bytes)),
            4 => format!("[{}]", self.argument),
            5 => format!("{}{}{}", "{", self.argument, "}"),
            6 => format!("{}()", self.argument),
            7 => format!("m{}", self.argument),
            _ => panic!("Invalid major type"),
        };

        write!(f, "{} @{}", type_line, self.start)
    }
}

fn parse_big_endian(bytes: &[u8], size: usize) -> Result<usize, ParseError> {
    if size > bytes.len() {
        return Err(ParseError::NotEnoughBytes);
    }

    let mut result = 0;
    for byte in &bytes[0..size] {
        result <<= 8;
        result |= *byte as usize;
    }

    Ok(result)
}

fn parse_bytes(bytes: &[u8], size: usize) -> Result<Vec<u8>, ParseError> {
    if size > bytes.len() {
        return Err(ParseError::NotEnoughBytes);
    }

    Ok(Vec::from(&bytes[0..size]))
}

#[derive(Debug)]
enum ParseError {
    TooManyBytes,
    NotEnoughBytes,
    ReservedArgument(usize),
    UnsupportedIndefiniteLength,
    MalformedInput,
}
*/
fn put<'a>(c: &'a cbor::Cbor, indent: usize) {
    let (description, children) = c.semantic();

    for _ in 0..=indent {
        print!("    ");
    }

    println!("{}", description);

    for child in children {
        put(&child, indent + 1);
    }
}

fn main() {
    // let bytes = [0x83, 0x01, 0x02, 0x03];
    let bytes = read_cbor();

    let bytes = [0x9f, 0x01, 0x9f, 0x02, 0x03, 0x04, 0xff, 0x05, 0xff];

    let c = cbor::Cbor::from_bytes(&bytes).unwrap();

    put(&c, 0);

    println!("{:#?}", c);
}

// fn read_cbor() -> Vec<u8> {
//     vec![
//         0xd8, 0x6b, 0xa2, 0x02, 0x58, 0x73, 0x82, 0x58, 0x24, 0x82, 0x2f, 0x58, 0x20, 0xa6, 0xc4,
//         0x59, 0x0a, 0xc5, 0x30, 0x43, 0xa9, 0x8e, 0x8c, 0x41, 0x06, 0xe1, 0xe3, 0x1b, 0x30, 0x55,
//         0x16, 0xd7, 0xcf, 0x0a, 0x65, 0x5e, 0xdd, 0xfa, 0xc6, 0xd4, 0x5c, 0x81, 0x0e, 0x03, 0x6a,
//         0x58, 0x4a, 0xd2, 0x84, 0x43, 0xa1, 0x01, 0x26, 0xa0, 0xf6, 0x58, 0x40, 0xd1, 0x1a, 0x2d,
//         0xd9, 0x61, 0x0f, 0xb6, 0x2a, 0x70, 0x73, 0x35, 0xf5, 0x84, 0x07, 0x92, 0x25, 0x70, 0x9f,
//         0x96, 0xe8, 0x11, 0x7e, 0x7e, 0xee, 0xd9, 0x8a, 0x2f, 0x20, 0x7d, 0x05, 0xc8, 0xec, 0xfb,
//         0xa1, 0x75, 0x52, 0x08, 0xf6, 0xab, 0xea, 0x97, 0x7b, 0x8a, 0x6e, 0xfe, 0x3b, 0xc2, 0xca,
//         0x32, 0x15, 0xe1, 0x19, 0x3b, 0xe2, 0x01, 0x46, 0x7d, 0x05, 0x2b, 0x42, 0xdb, 0x6b, 0x72,
//         0x87, 0x03, 0x58, 0x71, 0xa5, 0x01, 0x01, 0x02, 0x00, 0x03, 0x58, 0x5f, 0xa2, 0x02, 0x81,
//         0x81, 0x41, 0x00, 0x04, 0x58, 0x56, 0x86, 0x14, 0xa4, 0x01, 0x50, 0xfa, 0x6b, 0x4a, 0x53,
//         0xd5, 0xad, 0x5f, 0xdf, 0xbe, 0x9d, 0xe6, 0x63, 0xe4, 0xd4, 0x1f, 0xfe, 0x02, 0x50, 0x14,
//         0x92, 0xaf, 0x14, 0x25, 0x69, 0x5e, 0x48, 0xbf, 0x42, 0x9b, 0x2d, 0x51, 0xf2, 0xab, 0x45,
//         0x03, 0x58, 0x24, 0x82, 0x2f, 0x58, 0x20, 0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77,
//         0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff, 0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd,
//         0xef, 0xfe, 0xdc, 0xba, 0x98, 0x76, 0x54, 0x32, 0x10, 0x0e, 0x19, 0x87, 0xd0, 0x01, 0x0f,
//         0x02, 0x0f, 0x0a, 0x43, 0x82, 0x03, 0x0f, 0x0c, 0x43, 0x82, 0x17, 0x02,
//     ]
// }

fn read_cbor() -> Vec<u8> {
    vec![
        0x8a, 0x01, 0x18, 0x42, 0xd9, 0x01, 0x32, 0x21, 0x2b, 0xf5, 0xf4, 0xf6, 0xa2, 0x01, 0x64,
        0x54, 0x65, 0x73, 0x74, 0x81, 0x01, 0x65, 0x4f, 0x74, 0x68, 0x65, 0x72, 0x43, 0x82, 0x12,
        0x13, 0xfb, 0x3f, 0xf0, 0xf5, 0xc2, 0x8f, 0x5c, 0x28, 0xf6,
    ]
}

#[derive(Debug)]
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

    fn size(&self) -> usize {
        self.children.iter().fold(
            self.header_byte_count + self.body_byte_count,
            |acc, child| acc + child.size(),
        )
    }

    fn put(&self, indent: usize) {
        for _ in 0..indent {
            print!("    ");
        }

        println!("{}", self);

        for child in &self.children {
            child.put(indent + 1);
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

fn main() {
    let cbor = read_cbor();

    let root = Element::from_cbor(&cbor, 0).unwrap();

    root.put(0);

    println!("Root size: {}", root.size());
}

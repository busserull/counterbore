#[derive(Debug)]
pub struct Cbor<'a> {
    major: u8,
    additional_info: u8,
    argument: usize,

    start: usize,
    head_byte_count: usize,
    body_byte_count: usize,

    body_bytes: &'a [u8],
    children: Vec<Cbor<'a>>,
}

#[derive(Debug)]
pub enum ParseError {
    // Problem byte followed by ...
    // TooManyBytes(usize),
    TooFewBytes(usize, usize), // ... byte count lacking
    ReservedAdditionalInfo(usize, u8), // ... reserved info value
                               // MalformedInput(usize),
}

impl<'a> Cbor<'a> {
    pub fn from_bytes(bytes: &'a [u8]) -> Result<Self, ParseError> {
        Cbor::cbor_from_bytes(bytes, 0)
    }

    pub fn size(&self) -> usize {
        self.head_byte_count
            + self.body_byte_count
            + self.children.iter().fold(0, |acc, c| acc + c.size())
    }

    pub fn semantic(&self) -> (String, &Vec<Cbor<'a>>) {
        let description = match (self.major, self.argument) {
            (0, value) => format!("{}", value),
            (1, value) => format!("-{}", value + 1),
            (2, _) => format!("<<{:?}>>", self.body_bytes),
            (3, _) => String::from_utf8_lossy(self.body_bytes).into_owned(),
            (4, children) => format!("[{}]", children),
            (5, pairs) => format!("{{{}}}", pairs),
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
        for _ in 0..child_count {
            let child = Cbor::cbor_from_bytes(&bytes[child_offset..], start + child_offset)?;
            child_offset += child.size();
            children.push(child);
        }

        Ok(Self {
            major,
            additional_info,
            argument,

            start,
            head_byte_count,
            body_byte_count,

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

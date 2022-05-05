use std::env;
use std::fs;

mod cbor;
use cbor::Cbor;

fn main() {
    let filename = env::args()
        .skip(1)
        .next()
        .expect("Please supply a file to parse");

    let content: String = fs::read_to_string(filename)
        .expect("Could not read file")
        .chars()
        .filter(|point| point.is_ascii_hexdigit())
        .collect();

    let bytes = hex::decode(content).expect("File contains invalid hex data");

    match Cbor::from_bytes(&bytes) {
        Ok(cbor) => println!("{}", cbor),
        Err(error) => println!("{}", error),
    }
}

use std::fmt::Debug;

pub mod multipart;
pub mod quoted_printable;
pub mod base64;
pub mod spaceless;

pub trait Encoder {
    fn encode(input: &[u8]) -> String {
        let mut s = String::new();
        Self::encode_to_string(input, &mut s);
        s
    }

    fn encode_to_string(input: &[u8], res: &mut String) -> usize;
}

pub trait Decoder {
    type Error: Debug;

    fn decode(input: &[u8]) -> Result<String, Self::Error> {
        let mut s = String::new();
        Self::decode_to_string(input, &mut s)?;
        Ok(s)
    }

    fn decode_to_string(input: &[u8], res: &mut String) -> Result<usize, Self::Error>;

    // not all encoded values are valid strings...
    // fn decode_to_vec(input: &[u8], res: &mut Vec<u8>) -> Result<usize, Self::Error>;
}
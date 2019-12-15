pub use quoted::*;
pub use rfc_2047::*;

mod rfc_2047;
mod quoted;

#[derive(Debug, From)]
pub enum QuotedStringError {
    InputEmpty,
    FirstCharIsNotQuote,
    LastCharacterIsNotQuote,
    UnexpectedEof,
    InvalidEncoding,
    DecodingFailed,
    InvalidCharacter {
        char_offset: usize,
        byte_offset: usize,
    },
}

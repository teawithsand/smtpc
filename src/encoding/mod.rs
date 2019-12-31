//! Encoding module implements support for various encodings in streaming manner
//! right now `QuotedPrintable` and `Base64` are supported
//!
//! For multipart there is `PartReader`

pub mod multipart;
pub mod quoted_printable;
pub mod base64;
pub mod spaceless;
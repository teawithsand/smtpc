use std::borrow::Cow;
use std::io::Cursor;

use crate::encoding::{Decoder, Encoder};
use crate::encoding::base64::{Base64Decoder, Base64Encoder};
use crate::encoding::quoted_printable::{QuotedPrintableDecoder, QuotedPrintableEncoder};
use crate::utils::quoted::QuotedStringError;

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum RFC2047Encoding {
    Base64,
    QuotedPrintable,
}

impl RFC2047Encoding {
    // TODO(teawithsand) write fuzzer which tires to mismatch that

    /// encoded_len returns size of encoded data when given encoding would be used.
    /// It does not return length of entire result but length of encoded data.
    /// So:  encoded_length - constant_sized_parts
    pub fn encoded_len(&self, data: &str) -> u64 {
        match self {
            RFC2047Encoding::Base64 => {
                let n = (data.as_bytes().len() as u64) * 4 / 3;
                if n % 4 != 0 {
                    n + (4 - n % 4)
                } else {
                    n
                }
            }
            RFC2047Encoding::QuotedPrintable => data.chars().map(|c| match c {
                c if c.is_ascii() => 1,
                c => (c.len_utf8() * 3) as u64,
            }).sum(),
        }
    }

    pub fn rfc_letter(&self) -> &'static str {
        match self {
            RFC2047Encoding::Base64 => "B",
            RFC2047Encoding::QuotedPrintable => "Q",
        }
    }
}

/// optimal_encode_rfc_2047 encodes text using either base64 or quoted printable dependent on which result is smaller
pub fn optimal_encode_rfc_2047(text: &str) -> String {
    let qp_sz = RFC2047Encoding::QuotedPrintable.encoded_len(text);
    let b64sz = RFC2047Encoding::Base64.encoded_len(text);
    if b64sz <= qp_sz {
        encode_rfc_2047(text, RFC2047Encoding::Base64)
    } else {
        encode_rfc_2047(text, RFC2047Encoding::QuotedPrintable)
    }
}

pub fn encode_rfc_2047(text: &str, encoding: RFC2047Encoding) -> String {
    let mut res = String::new();
    res.push_str("=?UTF-8?");
    res.push_str(encoding.rfc_letter());
    res.push('?');
    match encoding {
        RFC2047Encoding::QuotedPrintable => {
            QuotedPrintableEncoder::encode_to_string(text.as_bytes(), &mut res);
        }
        RFC2047Encoding::Base64 => {
            Base64Encoder::encode_to_string(text.as_bytes(), &mut res);
        }
    }
    res.push_str("?=");

    res
}

pub fn parse_rfc_2047(text: &str) -> Result<String, QuotedStringError> {
    let mut state = 0;
    let mut charset = &text[..];
    let mut encoding = &text[..];
    let mut enc_text = &text[..];

    let mut charset_start_offset = 0;
    let mut encoding_start_offset = 0;
    let mut enc_text_start_offset = 0;
    let mut byte_offset = 0;
    for c in text.chars() {
        if state == 0 {
            if c == '=' {
                state = 1;
            } else {
                return Err(QuotedStringError::UnexpectedEof);
            }
        } else if state == 1 {
            if c == '?' {
                charset_start_offset = byte_offset;
                state = 2;
            } else {
                return Err(QuotedStringError::UnexpectedEof);
            }
        } else if state == 2 {
            if c == '?' {
                charset = &text[charset_start_offset..byte_offset];
                encoding_start_offset = byte_offset + 1;
                state = 3;
            }
        } else if state == 3 {
            if c == '?' {
                encoding = &text[encoding_start_offset..byte_offset];
                enc_text_start_offset = byte_offset + 1;
                state = 4;
            }
        } else if state == 4 {
            if c == '?' {
                state = 5;
            }
        } else if state == 5 {
            if c == '=' {
                enc_text = &text[enc_text_start_offset..byte_offset - 1];
                break;
            } else {
                state = 4;
            }
        } else {
            unreachable!("invalid state");
        }

        byte_offset += c.len_utf8();
    }
    if enc_text == text {
        return Err(QuotedStringError::UnexpectedEof);
    }
    let dec_res = if encoding == "B" || encoding == "b" {
        Base64Decoder::decode(enc_text.as_bytes())
        // read_stream_to_string(&mut QuotedPrintableReader::new(Cursor::new(enc_text.as_bytes())))
    } else if encoding == "Q" || encoding == "q" {
        QuotedPrintableDecoder::decode(enc_text.as_bytes())
        // read_stream_to_string(&mut Base64Reader::new(Cursor::new(enc_text.as_bytes())))
    } else {
        return Err(QuotedStringError::InvalidEncoding);
    }.map_err(|_| QuotedStringError::DecodingFailed)?;
    // ignore encoding. rust can support only utf8 in the end
    Ok(dec_res)
}

pub fn parse_maybe_rfc_2047(text: &str) -> Result<Cow<str>, QuotedStringError> {
    if text.is_empty() {
        Ok(Cow::Borrowed(text))
    } else if text.chars().nth(0) == Some('=') && text.chars().nth(1) == Some('?') {
        Ok(Cow::Owned(parse_rfc_2047(text)?))
    } else {
        Ok(Cow::Borrowed(text))
    }
}

pub fn parse_maybe_rfc_2047_is_encoded(text: &str) -> Result<(Cow<str>, bool), QuotedStringError> {
    let is_encoded = text.chars().nth(0) == Some('=') && text.chars().nth(1) == Some('?');
    let text = parse_maybe_rfc_2047(text)?;
    Ok((text, is_encoded))
}

#[cfg(test)]
mod test {
    use super::*;

    //noinspection SpellCheckingInspection
    #[test]
    fn test_encoded_len() {
        for (e, i, o) in [
            (RFC2047Encoding::QuotedPrintable, "", 0),
            (RFC2047Encoding::QuotedPrintable, "asdf", 4),
            (RFC2047Encoding::QuotedPrintable, "ŁŁŁŁ", 4 * 2 * 3),
            (RFC2047Encoding::QuotedPrintable, "aaŁŁŁŁ", 2 + 4 * 2 * 3),
            // // // //
            (RFC2047Encoding::Base64, "", 0),
            (RFC2047Encoding::Base64, "ŁŁ", 8),
            (RFC2047Encoding::Base64, "a", 4),
            (RFC2047Encoding::Base64, "aa", 4),
            (RFC2047Encoding::Base64, "aaa", 4),
            (RFC2047Encoding::Base64, "aaaa", 8),
            (RFC2047Encoding::Base64, "aaaaa", 8),
            (RFC2047Encoding::Base64, "aaaaaa", 8),
            (RFC2047Encoding::Base64, "aaaaaaa", 12),
            (RFC2047Encoding::Base64, "aaaaaaaa", 12),
            (RFC2047Encoding::Base64, "aaaaaaaaa", 12),
        ].iter() {
            eprintln!("Case: {:?} {:?} {:?}", e, i, o);
            assert_eq!(e.encoded_len(i), *o);
        }
    }

    #[test]
    fn test_can_parse_rfc_2047() {
        for (i, o) in [
            ("=?UTF-8?B?QUFB?=", Some("AAA")),
            ("=?UTF-8?Q?ABCD?=", Some("ABCD")),
            ("=?UTF-8?Q?aa=C5=81=C5=81?=", Some("aaŁŁ")),
            // // // // lowercase enc + type tests
            ("=?utf-8?b?QUFB?=", Some("AAA")),
            ("=?utf-8?q?ABCD?=", Some("ABCD")),
            ("=?utf-8?q?aa=C5=81=C5=81?=", Some("aaŁŁ")),
        ].iter() {
            if let Some(o) = o {
                let res = parse_rfc_2047(*i).unwrap();
                assert_eq!(&res, *o);
            } else {
                parse_rfc_2047(*i).unwrap_err();
            }
        }
    }

    #[test]
    fn test_can_encode_and_parse() {
        for i in [
            "",
            "a",
            "aaa",
            "aaŁŁ",
        ].iter() {
            let b64_res = encode_rfc_2047(i, RFC2047Encoding::Base64);
            let qp_res = encode_rfc_2047(i, RFC2047Encoding::QuotedPrintable);

            assert_eq!(&parse_rfc_2047(&b64_res).unwrap(), i);
            assert_eq!(&parse_rfc_2047(&qp_res).unwrap(), i);
        }
    }
}


use crate::utils::cc::*;
use crate::utils::quoted::QuotedStringError;

pub fn unquote_string(text: &str, contains_quotes: bool) -> Result<String, QuotedStringError> {
    if text.is_empty() && contains_quotes {
        return Err(QuotedStringError::InputEmpty);
    }
    let mut is_escaped = false;
    let mut out = String::with_capacity(text.len().checked_sub(2).unwrap_or(1));

    let mut char_offset = 0;
    let mut byte_offset = 0;
    for c in text.chars() {
        if byte_offset == 0 && contains_quotes {
            if c != '"' {
                return Err(QuotedStringError::FirstCharIsNotQuote);
            }
        } else {
            match c {
                // TODO(teawithsand) test this guard expression evaluation order
                '"' | '\\' if is_escaped => {
                    out.push(c);
                    is_escaped = false;
                }
                '"' if contains_quotes => {
                    break;
                }
                '\\' => {
                    is_escaped = true;
                }
                c if is_escaped && (is_qtext(c) || is_white_space(c)) => {
                    out.push(c);
                    is_escaped = false;
                }
                c if is_qtext(c) || is_white_space(c) => {
                    out.push(c);
                }
                _ => {
                    return Err(QuotedStringError::InvalidCharacter {
                        byte_offset,
                        char_offset,
                    });
                }
            }
        };
        char_offset += 1;
        byte_offset += c.len_utf8();
    }

    if !text.is_empty() && contains_quotes && text.as_bytes().len() - 1 != byte_offset {
        return Err(QuotedStringError::LastCharacterIsNotQuote);
    }

    Ok(out)
}

pub fn quote_string(text: &str, emit_quotes: bool) -> String {
    let mut out = String::with_capacity(text.len());
    if emit_quotes {
        out.push('"');
    }
    for c in text.chars() {
        match c {
            c if is_qtext(c) || is_white_space(c) => out.push(c),
            c if is_vchar(c) => {
                out.push('\\');
                out.push(c);
            }
            // invalid char to put. panic?
            c => out.push(c),
        }
    }
    if emit_quotes {
        out.push('"');
    }

    out
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_can_quote_string() {
        for (i, o, eq) in [
            (r#"abc"#, r#"abc"#, false),
            (r#"""#, r#"\""#, false),
            (r#"ł"#, r#"ł"#, false),
            // // // //
            (r#"abc"#, r#""abc""#, true),
        ].iter() {
            let res = quote_string(i, *eq);
            assert_eq!(o, &res);
        }
    }

    #[test]
    fn test_can_unquote_string() {
        for (i, o, cq) in [
            (r#""hi bob""#, Some("hi bob"), true),
            (concat!("\"", "abc\\\t", "\""), Some("abc\t"), true),
            ("", None, true),
            (r#""hi bob" some hidden data"#, None, true),
            (r#"some hidden data here "hi bob""#, None, true),
            // // // //
            ("", Some(""), false),
            ("abc", Some("abc"), false),
            ("abc\\\t", Some("abc\t"), false),
        ].iter() {
            if let Some(o) = o {
                let res = unquote_string(i, *cq).unwrap();
                assert_eq!(&res, *o);
            } else {
                unquote_string(i, *cq).unwrap_err();
            }
        }
    }
}
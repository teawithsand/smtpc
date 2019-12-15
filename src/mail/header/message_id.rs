use std::borrow::Cow;

use crate::utils::iter_ext::CharsOffsetEnumerate;

// not sure if this is right place for this module...

#[derive(Debug, From)]
pub enum MessageIDParseError {
    NoEntryFound,
    BracketNotClosed,
    InvalidCharBetweenBrackets,
    FoundMany,
}

// TODO(teawithsand) make it not parse all ids if many found
pub fn parse_message_id(text: &str) -> Result<Cow<str>, MessageIDParseError> {
    let mut res = parse_multiple_message_id(text)?;
    if res.len() > 1 {
        Err(MessageIDParseError::FoundMany)
    } else {
        // this one does not cause vector reallocation unlike remove
        Ok(res.swap_remove(0))
    }
}

pub fn parse_multiple_message_id(text: &str) -> Result<Vec<Cow<str>>, MessageIDParseError> {
    let text = text.trim();

    let mut state = 0;
    let mut previous_offset = 0;
    let mut results = Vec::new();
    for (byte_offset, c) in text.chars().utf8_offset_enumerate() {
        if state == 0 {
            if c == '<' {
                previous_offset = byte_offset;
                state = 1;
                continue;
            } else {
                return Err(MessageIDParseError::NoEntryFound);
            }
        } else if state == 1 {
            if c == '>' {
                let offset = byte_offset - '>'.len_utf8();
                results.push(Cow::Borrowed(&text[previous_offset..offset]));
                previous_offset = byte_offset;
                state = 2;
                continue;
            }
        } else if state == 2 {
            if c.is_whitespace() {
                previous_offset = byte_offset;
                continue;
            } else if c == '<' {
                previous_offset = byte_offset;
                state = 1;
            } else { // not whitespace char not between '<' and '>' brackets
                return Err(MessageIDParseError::InvalidCharBetweenBrackets);
            }
        } else {
            unreachable!("Invalid state");
        }
    }
    // zero state is no bracket found and one state is looking for bracket end
    // none of them is valid
    if state == 0 {
        return Err(MessageIDParseError::NoEntryFound);
    } else if state == 1 {
        return Err(MessageIDParseError::BracketNotClosed);
    }
    Ok(results)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_can_parse_single_message_id() {
        for (i, o) in [
            ("", None),
            ("asdf", None),
            ("<asdf>", Some("asdf")),
            ("  <asdf>  ", Some("asdf")),
            ("  < asdf >  ", Some(" asdf ")),
        ].iter() {
            if let Some(o) = o {
                assert_eq!(*o, parse_message_id(i).unwrap().as_ref());
            } else {
                parse_message_id(i).unwrap_err();
            }
        }
    }

    #[test]
    fn test_can_parse_multiple_message_ids() {
        for (i, o) in [
            ("", None),
            ("asdf", None),
            ("<asdf>", Some(vec![
                "asdf"
            ])),
            ("  <asdf>  ", Some(vec![
                "asdf"
            ])),
            ("  < asdf >  ", Some(vec![
                " asdf "
            ])),
            // // // // for many ids test
            ("<asdf> <asdf> <asdf>", Some(vec![
                "asdf",
                "asdf",
                "asdf",
            ])),
            (" \t <asdf>  <asdf>   <asdf>  ", Some(vec![
                "asdf",
                "asdf",
                "asdf",
            ])),
        ].iter() {
            if let Some(o) = o {
                assert_eq!(*o, parse_multiple_message_id(i).unwrap());
            } else {
                parse_message_id(i).unwrap_err();
            }
        }
    }
}

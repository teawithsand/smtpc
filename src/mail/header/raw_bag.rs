use std::borrow::Cow;
use std::collections::HashMap;

use crate::utils::cc::is_white_space;

#[derive(Debug, From)]
pub enum MailHeaderParseError {
    FirstCharInvalid,
    InvalidHeaderName,
    InvalidHeaderContent,
    InvalidHeaderValue,
}

// SMTP headers can't be read in reader because they contain multiline syntax:
// if line after given header starts with space then it's next line of previous header...

struct MailHeadersParser<'a> {
    text: &'a str,
}

fn is_char_valid_name_char(c: char) -> bool {
    match c {
        '!'..='\'' => true,
        '*' | '+' | '-' | '.' | '^' | '_' | '`' | '|' | '~' => true,
        '0'..='9' => true,
        // (teawithsand) check case(what is alpha?) issues according to https://golang.org/src/net/textproto/reader.go?s=15120:15164#L559
        'a'..='z' => true,
        'A'..='Z' => true,
        _ => false,
    }
}

fn canonicalize_header_name(text: &str) -> Result<Cow<str>, ()> {
    for c in text.chars() {
        if !is_char_valid_name_char(c) {
            // when can't canonicalize just don't
            return Err(());
        }
    }
    debug_assert!(text.chars().all(|c| c.is_ascii()));

    let mut do_upper_case = true;
    let mut res = Cow::Borrowed(text);
    for (i, c) in text.chars().enumerate() {
        if c.is_ascii_alphabetic() { // is upper/lower case makes any sense for given char
            if do_upper_case && c.is_ascii_lowercase() {
                let data = res.to_mut();
                data[i..=i].to_ascii_uppercase();
                // do_upper_case = false;
            } else if !do_upper_case && c.is_ascii_uppercase() {
                let data = res.to_mut();
                data[i..=i].to_ascii_lowercase();
            }
        }
        do_upper_case = c == '-';
    }

    Ok(res)
}

impl<'a> MailHeadersParser<'a> {
    pub fn new(text: &'a str) -> Self {
        Self {
            text,
        }
    }

    pub fn take_white_chars(&mut self) {
        self.text = self.text.trim_start();
    }

    /*
    pub fn take_spaces_and_tabs(&mut self) {
        self.text = self.text.trim_start_matches(|c| c == ' ' || c == '\t');
    }
    */

    pub fn consume(&mut self, c: char) -> bool {
        if let Some(d) = self.peek_char() {
            if d == c {
                self.take_char().expect("Take char can't fail. Peek succeed");
                true
            } else {
                false
            }
        } else {
            false
        }
    }
    /*
    pub fn take_newline(&mut self) -> bool {
        let first_char = self.text.chars().nth(0);
        let second_char = self.text.chars().nth(1);
        match (first_char, second_char) {
            (Some('\n'), None) | (Some('\n'), Some(_)) => {
                self.take_char().expect("First char is set. Take can't fail.");
                true
            }
            (Some('\r'), Some('\n')) => {
                self.take_char().expect("First char is set. Take can't fail.");
                self.take_char().expect("Second char is set. Take can't fail.");
                true
            }
            _ => {
                false
            }
        }
    }
    */

    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    pub fn peek_char(&mut self) -> Option<char> {
        self.text.chars().nth(0)
    }

    pub fn take_char(&mut self) -> Option<char> {
        if let Some(c) = self.peek_char() {
            self.text = &self.text[c.len_utf8()..];
            Some(c)
        } else {
            None
        }
    }

    /// take_header_name reads header name(AKA key)
    /// Returned header name is canonical.
    pub fn take_header_name(&mut self) -> Result<Cow<'a, str>, MailHeaderParseError> {
        let mut byte_offset = 0;
        let text = self.text;
        loop {
            let c = match self.take_char() {
                Some(c) => c,
                None => {
                    return Err(MailHeaderParseError::InvalidHeaderName);
                }
            };
            if byte_offset == 0 && is_white_space(c) {
                return Err(MailHeaderParseError::InvalidHeaderName);
            }

            if c == ':' {
                break;
            }
            if !is_char_valid_name_char(c) {
                return Err(MailHeaderParseError::InvalidHeaderName);
            }
            byte_offset += c.len_utf8();
        }
        let text = text[..byte_offset].trim_start_matches(|c| is_white_space(c));
        if text.is_empty() {
            return Err(MailHeaderParseError::InvalidHeaderName);
        }
        Ok(canonicalize_header_name(text).map_err(|_| MailHeaderParseError::InvalidHeaderName)?)
    }

    pub fn take_header_value(&mut self) -> Result<Cow<'a, str>, MailHeaderParseError> {
        self.take_white_chars();

        // use following new line detection strategy:
        // 1. Find \n
        // 2. If char before it \r then rm it
        let mut byte_offset = 0;
        let text = self.text;
        loop {
            let c = match self.take_char() {
                Some(c) => c,
                None => {
                    // eof is fine.
                    break;
                    // return Err(MailHeaderParseError::InvalidHeaderValue);
                }
            };
            if c == '\n' {
                break;
            }
            byte_offset += c.len_utf8();
        }
        if byte_offset == 0 {
            return Err(MailHeaderParseError::InvalidHeaderContent);
        }
        let text = &text[..byte_offset];
        // use as bytes to not violate utf8 char boundary
        let text = if text.as_bytes()[text.as_bytes().len() - 1] == b'\r' {
            &text[..text.len() - 1]
        } else {
            text
        };

        Ok(Cow::Borrowed(text))
    }

    pub fn take_header_name_and_value(&mut self) -> Result<(Cow<'a, str>, Cow<'a, str>), MailHeaderParseError> {
        let name = self.take_header_name()?;
        let mut values = vec![self.take_header_value()?];
        loop {
            // take header value took newline. no check here.
            if self.consume(' ') {
                let rd_value = self.take_header_value()?;
                // TODO(teawithsand) how join multiline headers? With space char? No char?
                values.push(rd_value);
            } else {
                break;
            }
        }

        if values.len() == 1 {
            Ok((name, values.remove(0)))
        } else {
            Ok((name, Cow::Owned(values.join(""))))
        }
    }
}


#[derive(Debug, Clone, PartialEq)]
#[derive(From, Into)]
pub struct RawMailHeaderBag<'a> {
    // container: HashMap<&'a str, ParsedMailHeader<'a>>
    container: HashMap<Cow<'a, str>, Vec<Cow<'a, str>>>,
}

impl<'a> RawMailHeaderBag<'a> {
    #[inline]
    pub fn container(&self) -> &HashMap<Cow<'a, str>, Vec<Cow<'a, str>>> {
        &self.container
    }

    pub fn parse(text: &'a str) -> Result<Self, MailHeaderParseError> {
        let text = text.trim();
        let mut parser = MailHeadersParser::new(text);
        if parser.is_empty() {
            return Ok(Self {
                container: HashMap::new(),
            });
        }

        let mut res: HashMap<Cow<'a, str>, Vec<Cow<'a, str>>> = HashMap::new();
        match parser.peek_char() {
            None | Some(' ') => {
                return Err(MailHeaderParseError::InvalidHeaderValue);
            }
            Some(_) => {}
        };
        loop {
            if parser.is_empty() {
                break;
            }
            let (key, value) = parser.take_header_name_and_value()?;
            // looks like borrow checker is too stupid to use HashMap.entry().
            // Value has to be moved in one place and in the other one at the same time.
            if res.contains_key(&key) {
                res.get_mut(&key).unwrap().push(value);
            } else {
                res.insert(key, vec![value]);
            }
        }
        Ok(Self {
            container: res,
        })
    }

    pub fn new() -> Self {
        Self {
            container: HashMap::new(),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    macro_rules! map (
        { $($key:expr => $value:expr),+ } => {
            {
                let mut m = ::std::collections::HashMap::new();
                $(
                    m.insert($key, $value);
                )+
                m
            }
         };
    );

    #[test]
    fn test_can_parse_headers() {
        for (i, o) in [
            (
                "Subject: Test",
                Some(map! {
                    Cow::Borrowed("Subject") => vec![Cow::Borrowed("Test")]
                })
            ),
            (
                "Subject: Test\r\n",
                Some(map! {
                    Cow::Borrowed("Subject") => vec![Cow::Borrowed("Test")]
                })
            ),
            (
                "Subject: Test\n",
                Some(map! {
                    Cow::Borrowed("Subject") => vec![Cow::Borrowed("Test")]
                })
            ),
            (
                "Subject: Test\nSubject: Test",
                Some(map! {
                    Cow::Borrowed("Subject") => vec![Cow::Borrowed("Test"), Cow::Borrowed("Test")]
                })
            ),
            (
                "Subject: Test\r\nSubject: Test",
                Some(map! {
                    Cow::Borrowed("Subject") => vec![Cow::Borrowed("Test"), Cow::Borrowed("Test")]
                })
            ),
            (
                "Subject: Test\r\n Test",
                Some(map! {
                    Cow::Borrowed("Subject") => vec![Cow::Borrowed("TestTest")]
                })
            ),
            (
                "Subject: Test\r\n      Test\r\nSubject: asdf",
                Some(map! {
                    Cow::Borrowed("Subject") => vec![Cow::Borrowed("TestTest"), Cow::Borrowed("asdf")]
                })
            ),
            (
                "Subject: Test\n      Test\nSubject: asdf",
                Some(map! {
                    Cow::Borrowed("Subject") => vec![Cow::Borrowed("TestTest"), Cow::Borrowed("asdf")]
                })
            ),
        ].iter().cloned() {
            if let Some(o) = o {
                let c: HashMap<_, _> = RawMailHeaderBag::parse(i).unwrap().into();
                assert_eq!(c, o);
            } else {
                RawMailHeaderBag::parse(i).unwrap_err();
            }
        }
    }
}
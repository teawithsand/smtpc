use std::borrow::Cow;
use std::str::FromStr;

use crate::utils::cc::is_atext;
use crate::utils::quoted::{
    parse_maybe_rfc_2047,
    parse_maybe_rfc_2047_is_encoded,
    parse_rfc_2047,
    QuotedStringError,
    RFC2047Encoding,
    unquote_string,
};

// Took from https://github.com/golang/go/blob/master/src/net/mail/message.go
// TODO(teawithsand) add info about source stuff

#[derive(Debug, Clone, PartialEq)]
#[derive(Serialize, Deserialize)]
pub struct EmailAddress {
    pub name: String,
    pub address: String,
}

#[derive(Debug, From)]
pub enum EmailAddressParseError {
    InputEmpty,
    InvalidAtom,
    InvalidAddressSpec,
    InvalidQuotedString,
    InvalidComment,
    InvalidGroup,
}

struct AddressParser<'a> {
    address: &'a str,
}

impl<'a> AddressParser<'a> {
    pub fn new(address: &'a str) -> Self {
        Self {
            address: address.trim(),
        }
    }

    pub fn take_quoted_string(&mut self, take_quotes: bool) -> Result<String, EmailAddressParseError> {
        let mut byte_offset = 0;
        let mut state = if take_quotes { 0 } else { 1 };
        let mut found_end_quote = false;
        for c in self.address.chars() {
            if state == 0 {
                if c == '"' {
                    // eprintln!("First char is quote!");
                    state = 1;
                } else {
                    return Err(EmailAddressParseError::InvalidQuotedString);
                }
            } else if state == 1 {
                if c == '\\' {
                    state = 2;
                } else if c == '"' && take_quotes {
                    found_end_quote = true;
                    break;
                }
            } else if state == 2 {
                state = 1;
            } else {
                unreachable!("invalid state");
            }
            byte_offset += c.len_utf8();
        }
        if !found_end_quote && take_quotes {
            return Err(EmailAddressParseError::InvalidQuotedString);
        }
        debug_assert!(byte_offset <= self.address.len());
        // eprintln!("Unquoting: {:?}", &self.address[1..byte_offset]);
        let res = match unquote_string(&self.address[if take_quotes { 1 } else { 0 }..byte_offset], false) {
            Ok(v) => {
                // panic!("Unquoting succeed! {:?}", v);
                v
            }
            Err(_) => {
                return Err(EmailAddressParseError::InvalidQuotedString);
            }
        };
        self.address = &self.address[if take_quotes { 1 } else { 0 } + byte_offset..];
        // panic!("new str: `{}`", self.address);
        Ok(res)
    }

    pub fn take_white_chars(&mut self) {
        self.address = self.address.trim_start();
    }

    pub fn take_cfws(&mut self) -> bool {
        self.take_white_chars();
        loop {
            if !self.consume_char('(') {
                break true;
            }

            if !self.take_comment(true).is_ok() {
                break false;
            }

            self.take_white_chars();
        }
    }

    pub fn take_char(&mut self) -> Option<char> {
        if let Some(chr) = self.peek_char() {
            self.address = &self.address[chr.len_utf8()..];
            Some(chr)
        } else {
            None
        }
    }

    pub fn consume_char(&mut self, c: char) -> bool {
        match self.peek_char() {
            Some(ch) => {
                if ch == c {
                    self.take_char().expect("Peek succeed so take can't fail");
                    true
                } else {
                    false
                }
            }
            None => false
        }
    }

    pub fn peek_char(&mut self) -> Option<char> {
        if let Some(last_char) = self.address.chars().nth(0) {
            Some(last_char)
        } else {
            None
        }
    }

    pub fn is_empty(&self) -> bool {
        self.address.is_empty()
    }

    pub fn take_atom(&mut self, dot: bool, permissive: bool) -> Result<&'a str, EmailAddressParseError> {
        let mut byte_offset = 0;
        for c in self.address.chars() {
            if !is_atext(c, dot, permissive) {
                break;
            }
            byte_offset += c.len_utf8();
        }
        let atom = &self.address[..byte_offset];
        if byte_offset == 0 {
            return Err(EmailAddressParseError::InvalidAtom);
        }
        if !permissive && (
            atom.starts_with('.') ||
                atom.contains("..") ||
                atom.ends_with('.')
        ) {
            return Err(EmailAddressParseError::InvalidAtom);
        }

        self.address = &self.address[byte_offset..];
        Ok(atom)
    }

    pub fn take_phrase(&mut self) -> Result<String, EmailAddressParseError> {
        let mut words = Vec::new();
        let mut is_prev_encoded = false;
        let mut err = Ok(());
        loop {
            self.take_white_chars();
            let (word, is_encoded) = match self.peek_char() {
                None => break,
                Some('"') => {
                    match self.take_quoted_string(true) {
                        Ok(v) => {
                            // eprintln!("Took quoted string: {:?}", v);
                            (v, false)
                        }
                        Err(e) => {
                            // eprintln!("Err take quoted string!: {:?}", ap.address);
                            err = Err(e);
                            break;
                        }
                    }
                }
                Some(_) => {
                    match self.take_atom(true, true)
                        .map(|v| parse_maybe_rfc_2047_is_encoded(v)) {
                        Ok(Ok((s, b))) => {
                            // eprintln!("Taken atom: {:?} Parsed: {:?}", s.as_ref(), b);
                            (s.to_string(), b)
                        }
                        _ => {
                            // eprintln!("Field to take an atom: {:?}", ap.address);
                            err = Err(EmailAddressParseError::InvalidAtom);
                            break;
                        }
                    }
                }
            };
            if is_prev_encoded && is_encoded {
                let words_sz = words.len();
                words[words_sz - 1] = format!("{}{}", words[words.len() - 1], word);
            } else {
                words.push(word);
            }
            is_prev_encoded = is_encoded;
        }

        if words.is_empty() && err.is_err() {
            return Err(err.unwrap_err());
        }

        Ok(words.join(" "))
    }

    pub fn take_address_spec_may_rollback(&mut self) -> Result<String, EmailAddressParseError> {
        let mut ea = self.address;
        match self.take_address_spec() {
            Err(e) => {
                self.address = ea;
                Err(e)
            }
            Ok(v) => {
                Ok(v)
            }
        }
    }

    pub fn take_address_spec(&mut self) -> Result<String, EmailAddressParseError> {
        let local_part = match self.peek_char() {
            Some('"') => {
                Cow::Owned(self.take_quoted_string(true)?)
            }
            _ => {
                Cow::Borrowed(self.take_atom(true, false)?)
            }
        };
        if self.is_empty() {
            return Err(EmailAddressParseError::InvalidAddressSpec);
        }
        match self.take_char() {
            Some('@') => {
                // ok
            }
            None | Some(_) => {
                return Err(EmailAddressParseError::InvalidAddressSpec);
            }
        }
        self.take_white_chars();
        if self.address.is_empty() {
            return Err(EmailAddressParseError::InvalidAddressSpec);
        }
        let domain = self.take_atom(true, false).map(Ok)
            .unwrap_or(Err(EmailAddressParseError::InvalidAtom))?;
        let mut res = String::new();
        res.push_str(&local_part);
        res.push('@');
        res.push_str(&domain);
        Ok(res)
    }

    pub fn take_display_name_comment(&mut self) -> Result<String, EmailAddressParseError> {
        match self.take_char() {
            Some('(') => {}
            None | Some(_) => {
                return Err(EmailAddressParseError::InvalidComment);
            }
        }
        let comment = self.take_comment(true)?;
        let splits: Result<Vec<_>, _> = comment.split(' ')
            .flat_map(|s| s.split('\t'))
            .map(|r| parse_maybe_rfc_2047(r))
            .collect();
        let splits = splits.map_err(|_| EmailAddressParseError::InvalidComment)?;

        let res = splits.join(" ");
        Ok(res)
    }

    pub fn take_comment(&mut self, is_first_consumed: bool) -> Result<String, EmailAddressParseError> {
        // TODO(teawithsand) fix loop behaviour when flag is set to false
        let mut depth = if is_first_consumed { 1 } else { 0 };
        let mut res = String::new();
        loop {
            if self.address.is_empty() || depth == 0 {
                break;
            }
            if let Some(chr) = self.take_char() {
                if chr == '(' {
                    depth += 1;
                } else if chr == ')' {
                    depth -= 1;
                } else if chr == '\\' {
                    self.take_char().map(Ok)
                        .unwrap_or(Err(EmailAddressParseError::InvalidComment))?;
                    continue;
                }

                if depth > 0 {
                    res.push(chr);
                }
            } else {
                return Err(EmailAddressParseError::InvalidComment);
            }
        }
        if depth != 0 {
            return Err(EmailAddressParseError::InvalidComment);
        }
        Ok(res)
    }

    pub fn take_address(&mut self, allow_many: bool) -> Result<Vec<EmailAddress>, EmailAddressParseError> {
        if self.is_empty() {
            return Err(EmailAddressParseError::InputEmpty);
        }
        self.take_white_chars();

        if let Ok(spec) = self.take_address_spec_may_rollback() {
            self.take_white_chars();
            let dn = if self.peek_char() == Some('(') {
                self.take_display_name_comment()?
            } else {
                String::new()
            };
            return Ok(vec![EmailAddress {
                name: dn,
                address: spec,
            }]);
        }
        // not an addr-spec address.

        let dn = match self.peek_char() {
            None => {
                return Err(EmailAddressParseError::InvalidComment);
            }
            Some('<') => {
                String::new()
            }
            Some(_) => {
                self.take_phrase()?
            }
        };
        self.take_white_chars();

        if allow_many && self.consume_char(':') {
            return self.take_group_list();
        }

        match self.peek_char() {
            None => {
                return Err(EmailAddressParseError::InvalidAtom);
            }
            Some('<') => {
                self.take_char().expect("Peek succeed. Take can't fail.");
            }
            Some(_) => {
                if !dn.chars().all(|c| is_atext(c, true, false)) {
                    return Err(EmailAddressParseError::InvalidAtom); // TODO(teawithsand) proper error
                }
                return Err(EmailAddressParseError::InvalidAtom); // TODO(teawithsand) proper error
            }
        };

        let spec = self.take_address_spec()?;
        match self.peek_char() {
            None => {
                return Err(EmailAddressParseError::InvalidAddressSpec);
            }
            Some('>') => {
                self.take_char().expect("Peek succeed. Take can't fail.");
            }
            Some(c) => {
                return Err(EmailAddressParseError::InvalidAddressSpec);
            }
        };

        Ok(vec![EmailAddress {
            name: dn,
            address: spec,
        }])
    }

    pub fn take_group_list(&mut self) -> Result<Vec<EmailAddress>, EmailAddressParseError> {
        self.take_white_chars();
        if self.consume_char(';') {
            self.take_cfws();
            return Ok(Vec::new());
        }
        let mut res = Vec::new();
        loop {
            self.take_white_chars();
            let address = self.take_address(false)?;
            debug_assert_eq!(address.len(), 1, "Take address with multiple false returns either one address or error");
            res.extend_from_slice(&address[..]);

            if !self.take_cfws() {
                return Err(EmailAddressParseError::InvalidGroup);
            }

            if self.consume_char(';') {
                self.take_cfws();
                break;
                // return Err(EmailAddressParseError::InvalidGroup);
            }

            if !self.consume_char(',') {
                return Err(EmailAddressParseError::InvalidGroup);
            }
        }
        Ok(res)
    }

    pub fn take_address_list(&mut self) -> Result<Vec<EmailAddress>, EmailAddressParseError> {
        let mut res = Vec::new();
        loop {
            self.take_white_chars();
            let address = self.take_address(true)?;
            // debug_assert_eq!(address.len(), 1, "Take address with multiple false returns either one address or error");
            res.extend_from_slice(&address[..]);

            if !self.take_cfws() {
                return Err(EmailAddressParseError::InvalidGroup);
            }

            if self.is_empty() {
                break;
            }

            if !self.consume_char(',') {
                return Err(EmailAddressParseError::InvalidGroup);
            }
        }
        Ok(res)
    }
}

impl EmailAddress {
    pub fn parse_single(address: &str) -> Result<Self, EmailAddressParseError> {
        let mut pa = AddressParser::new(address);
        let mut res = pa.take_address(false)?;
        assert_eq!(res.len(), 1, "Take address with multiple false returns either one address or error");
        Ok(res.remove(0))
    }

    pub fn parse_group(addresses: &str) -> Result<Vec<Self>, EmailAddressParseError> {
        let mut pa = AddressParser::new(addresses);
        pa.take_address_list()
    }
}

impl FromStr for EmailAddress {
    type Err = EmailAddressParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse_single(s)
    }
}


// TODO(teawithsand) remove below fns
pub fn parse_address(address: &str) -> Result<EmailAddress, EmailAddressParseError> {
    let mut pa = AddressParser::new(address);
    let mut res = pa.take_address(false)?;
    assert_eq!(res.len(), 1, "Take address with multiple false returns either one address or error");
    Ok(res.remove(0))
}

pub fn parse_address_group(address: &str) -> Result<Vec<EmailAddress>, EmailAddressParseError> {
    let mut pa = AddressParser::new(address);
    pa.take_address_list()
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_can_parse_single_email_address() {
        for (i, o) in [
            ("", None),
            ("\"", None),
            ("asdf@example.com", Some(EmailAddress {
                name: "".to_string(),
                address: "asdf@example.com".to_string(),
            })),
            ("John Doe <asdf@example.com>", Some(EmailAddress {
                name: "John Doe".to_string(),
                address: "asdf@example.com".to_string(),
            })),
            ("John J. Doe <asdf.j.fdsa@example.com>", Some(EmailAddress {
                name: "John J. Doe".to_string(),
                address: "asdf.j.fdsa@example.com".to_string(),
            })),
            ("John (middle) Doe <asdf@example.com>", Some(EmailAddress {
                name: "John (middle) Doe".to_string(),
                address: "asdf@example.com".to_string(),
            })),
            ("\"John (middle) Doe\" <asdf@example.com>", Some(EmailAddress {
                name: "John (middle) Doe".to_string(),
                address: "asdf@example.com".to_string(),
            })),
            ("\"John <middle> Doe\" <asdf@example.com>", Some(EmailAddress {
                name: "John <middle> Doe".to_string(),
                address: "asdf@example.com".to_string(),
            })),
            ("John !@M@! Doe <asdf@example.com>", Some(EmailAddress {
                name: "John !@M@! Doe".to_string(),
                address: "asdf@example.com".to_string(),
            })),
            ("John Doe <asdf@example.com> (asdf)", Some(EmailAddress {
                name: "John Doe".to_string(),
                address: "asdf@example.com".to_string(),
            })),
            ("asdf@example.com(John Doe)", Some(EmailAddress {
                name: "John Doe".to_string(),
                address: "asdf@example.com".to_string(),
            })),
            ("asdf@example.com (John Doe)", Some(EmailAddress {
                name: "John Doe".to_string(),
                address: "asdf@example.com".to_string(),
            })),
            ("<asdf@example.com> (CFWS (cfws))  (another comment)", Some(EmailAddress {
                name: "".to_string(),
                address: "asdf@example.com".to_string(),
            })),
        ].iter().cloned() {
            // eprintln!("Input: {:?}", i);
            if let Some(o) = o {
                let o = vec![o];

                let pa = parse_address(i).unwrap();
                assert_eq!(o, vec![pa]);

                // group is able to parse single address as well
                let pag = parse_address_group(i).unwrap();
                assert_eq!(o, pag);
            } else {
                parse_address(i).unwrap_err();
                parse_address_group(i).unwrap_err();
            }
        }
    }

    #[test]
    fn test_can_parse_address_group() {
        for (i, o) in [
            ("Jane Doe <jane@example.com>, jdoe@example.org, John Doe <john@example.com>", Some(vec![
                EmailAddress {
                    address: "jane@example.com".to_string(),
                    name: "Jane Doe".to_string(),
                },
                EmailAddress {
                    address: "jdoe@example.org".to_string(),
                    name: "".to_string(),
                },
                EmailAddress {
                    address: "john@example.com".to_string(),
                    name: "John Doe".to_string(),
                },
            ]))
        ].iter() {
            if let Some(o) = o {
                let pag = parse_address_group(i).unwrap();
                assert_eq!(o, &pag);
            } else {
                parse_address_group(i).unwrap_err();
            }
        }
    }
}
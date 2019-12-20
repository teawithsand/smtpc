use std::borrow::Cow;
use std::collections::HashMap;
use std::str::FromStr;

use mime::FromStrError;

use crate::mail::address::{EmailAddress, EmailAddressParseError};
use crate::mail::header::{ContentTransferEncoding, MessageIDParseError, parse_message_id, parse_multiple_message_id, RawMailHeaderBag};
use crate::utils::quoted::{parse_maybe_rfc_2047, QuotedStringError};

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedHeaderBag<'a> {
    container: HashMap<Cow<'a, str>, Vec<ParsedMailHeader<'a>>>,
}

impl<'a> ParsedHeaderBag<'a> {
    pub fn parse_raw_bag(bag: &'a RawMailHeaderBag<'a>) -> Self {
        let mut c= HashMap::new();
        for (k, v) in bag.container().iter() {
            let mut res_vec = Vec::with_capacity(v.len());
            for val in v {
                res_vec.push(ParsedMailHeader::try_parse(k, val.as_ref()));
            }
            c.insert(Cow::Borrowed(k.as_ref()), res_vec);
        }
        Self {
            container: c
        }
    }

    #[inline]
    pub fn container(&self) -> &HashMap<Cow<'a, str>, Vec<ParsedMailHeader<'a>>>{
        &self.container
    }
}

#[derive(Debug, From)]
pub enum MailHeaderParseError {
    TypeNotMatched,
    QuotedStringError(QuotedStringError),
    EmailAddressParseError(EmailAddressParseError),
    MessageIDParseError(MessageIDParseError),
    MimeError(FromStrError),
}

#[derive(Debug, Clone, PartialEq)]
#[derive(Serialize, Deserialize)]
pub enum ParsedMailHeader<'a> {
    Subject(Cow<'a, str>),

    ReplyTo(EmailAddress),
    ReturnPath(EmailAddress),
    EnvelopeTo(EmailAddress),

    Bcc(Vec<EmailAddress>),
    Cc(Vec<EmailAddress>),
    To(Vec<EmailAddress>),

    From(EmailAddress),

    MessageID(Cow<'a, str>),

    InReplyTo(Cow<'a, str>),
    References(Vec<Cow<'a, str>>),

    // for example: Content-Type: multipart/form-data; boundary=some_boundary
    // first one is content type
    ContentType(
        // for text/plain+xml; encoding=utf8
        Cow<'a, str>, // text
        Cow<'a, str>, // plain
        Option<Cow<'a, str>>, // xml
        HashMap<Cow<'a, str>, Vec<Cow<'a, str>>>, // encoding=utf8
    ),
    ContentTransferEncoding(ContentTransferEncoding),
    // language may be in somewhat parsed/normalized form?
    ContentLanguage(Cow<'a, str>),
    // Date() // TODO(teawithsand) implement this

    // TODO(teawithsand) DKIM header
    UnknownHeader(&'a str),
}

impl<'a> ParsedMailHeader<'a> {
    pub fn try_parse(name: &str, content: &'a str) -> Self {
        match Self::parse(name, content) {
            Ok(s) => s,
            Err(_) => ParsedMailHeader::UnknownHeader(content)
        }
    }

    // name is assumed to be canonical
    pub fn parse(name: &str, content: &'a str) -> Result<Self, MailHeaderParseError> {
        // TODO(teawithsand) introduce case insensitive compare and then accept form which may be not normalized
        //  For instance both `Subject` and `subject` are valid names for smtp headers but first one is normalized
        let name = name.to_ascii_lowercase();
        match &name[..] {
            "subject" => {
                Ok(ParsedMailHeader::Subject(parse_maybe_rfc_2047(content)?))
            }
            "return-path" => {
                Ok(ParsedMailHeader::ReturnPath(EmailAddress::parse_single(content)?))
            }
            "envelope-to" => {
                Ok(ParsedMailHeader::ReturnPath(EmailAddress::parse_single(content)?))
            }
            "reply-to" => {
                Ok(ParsedMailHeader::ReplyTo(EmailAddress::parse_single(content)?))
            }
            "bcc" => {
                Ok(ParsedMailHeader::Bcc(EmailAddress::parse_group(content)?))
            }
            "cc" => {
                Ok(ParsedMailHeader::Cc(EmailAddress::parse_group(content)?))
            }
            "co" => {
                Ok(ParsedMailHeader::To(EmailAddress::parse_group(content)?))
            }
            "from" => {
                Ok(ParsedMailHeader::From(EmailAddress::parse_single(content)?))
            }
            "message-ID" => {
                Ok(ParsedMailHeader::MessageID(parse_message_id(content)?))
            }
            "in-reply-to" => {
                Ok(ParsedMailHeader::InReplyTo(parse_message_id(content)?))
            }
            "references" => {
                Ok(ParsedMailHeader::References(parse_multiple_message_id(content)?))
            }
            "content-type" => {
                let mime = mime::Mime::from_str(content)?;
                let mut v: HashMap<_, Vec<_>> = HashMap::new();
                for (name, value) in mime.params().map(|(p1, p2)| (p1.to_string(), p2.to_string())) {
                    let k = Cow::Owned(name);
                    if let Some(val) = v.get_mut(&k) {
                        val.push(Cow::Owned(value));
                    } else {
                        v.insert(k, vec![Cow::Owned(value)]);
                    }
                }
                let res = ParsedMailHeader::ContentType(
                    Cow::Owned(mime.type_().as_str().to_string()),
                    Cow::Owned(mime.subtype().as_str().to_string()),
                    mime.suffix().map(|s| Cow::Owned(s.as_str().to_string())),
                    v,
                );
                Ok(res)
            }
            "content-transfer-encoding" => {
                Ok(ParsedMailHeader::ContentTransferEncoding(ContentTransferEncoding::decode(content)))
            }
            "content-language" => {
                Ok(ParsedMailHeader::ContentLanguage(Cow::Borrowed(content)))
            }
            _ => Err(MailHeaderParseError::TypeNotMatched)
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    // TODO(teawithsand) tests for parsed mail bag

    #[test]
    fn test_can_parse_valid_header() {
        for (n, c, o) in [
            ("Content-Type", "application/binary", Some(ParsedMailHeader::ContentType(
                Cow::Borrowed("application"),
                Cow::Borrowed("binary"),
                None,
                HashMap::new(),
            ))),
            ("Content-Transfer-Encoding", "binary", Some(ParsedMailHeader::ContentTransferEncoding(
                ContentTransferEncoding::Binary,
            ))),
            ("Content-Transfer-Encoding", "blah", Some(ParsedMailHeader::ContentTransferEncoding(
                ContentTransferEncoding::Other,
            ))),
            ("Subject", "hello", Some(ParsedMailHeader::Subject(
                Cow::Borrowed("hello")
            ))),
            ("Subject", "=?UTF-8?Q?hello?=", Some(ParsedMailHeader::Subject(
                Cow::Borrowed("hello")
            ))),
            ("Subject", "=?UTF-8?B?qq?=", None),
        ].iter().cloned() {
            let res = ParsedMailHeader::parse(n, c);
            if let Some(o) = o {
                assert_eq!(o, res.unwrap());
            } else {
                res.unwrap_err();
            }
        }
    }
}
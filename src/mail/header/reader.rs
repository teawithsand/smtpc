use std::borrow::Cow;
use std::cmp::min;
use std::io::{Cursor, Error, ErrorKind, Read};

use crate::utils::{BoundaryDetector, BoundaryDetectorResult};

/// count_header_bytes tries to answer how many bytes of message are consumed by headers
/// using `MailHeaderReader`.
///
/// # Ok returns
/// It *DOES NOT* count `\r\n\r\n` sequence at the end of message.
/// It does not count character by bytes so index is safe to be used with string slicing syntax
///
/// # Error
/// It returns `Err(())` when `MailHeaderReader` detects invalid header structure
///
/// # Example
/// ```rust
///const SIMPLE_MAIL: &str = "\
///From: sender@example.com\r\n\
///To: recipient@example.com\r\n\
///Subject: Test\r\n\
///\r\n\
///This one is plain old email with no fancy features like multipart stuff.";
///
///const SIMPLE_MAIL_HEADERS: &str = "\
///From: sender@example.com\r\n\
///To: recipient@example.com\r\n\
///Subject: Test\
///";
/// use smtpc::mail::header::count_header_bytes;
/// let headers_size = count_header_bytes(SIMPLE_MAIL.as_bytes()).unwrap();
/// assert_eq!(&SIMPLE_MAIL[..headers_size], SIMPLE_MAIL_HEADERS);
/// ```
pub fn count_header_bytes(mail: &[u8]) -> Result<usize, ()> {
    let r = Cursor::new(mail); // no need for buffered reader, cursor is already fast
    let mut r = MailHeaderReader::new(r, true);
    let mut offset = 0;
    let mut buf = [0u8; 512]; // do not zero buffer on each iteration
    loop {
        match r.read(&mut buf) {
            Ok(0) => { // we are done without any error!
                debug_assert!(offset <= mail.len());
                return Ok(offset);
            }
            Ok(sz) => {
                offset += sz;
            }
            Err(_) => {
                return Err(());
            }
        }
    }
}

/// MailHeaderReader is able to separate mail's body part from it's header part.
/// It reads mail until it find `"\r\n\r\n"` sequence byte by byte.
///
/// Once this sequence is found it sets finished flag and returns `Ok(0)` on read tries.
/// Reader then may be recovered and used to read mail body. \r\n\r\n sequence will be already consumed.
pub struct MailHeaderReader<R> {
    reader: R,
    bd: BoundaryDetector<'static>,
    is_in_mail: bool,
    is_unexpected_eof: bool,
    is_finished: bool,

    rd_buf: [u8; 5],
    rd_buf_sz: u8,
}

// TODO(teawithsand) allow \n boundary not only \r\n. Do it in transparently without flags
impl<R> MailHeaderReader<R> {
    // TODO(teawithsand) implement support for single new line(\n instead of \r\n)
    pub fn new(reader: R, in_mail: bool) -> Self {
        Self {
            reader,
            is_in_mail: in_mail,
            is_unexpected_eof: false,
            is_finished: false,
            rd_buf: [0u8; 5],
            rd_buf_sz: 0,
            bd: BoundaryDetector::new(Cow::Borrowed(b"\r\n\r\n")),
        }
    }

    pub fn is_done(&self) -> bool {
        self.is_finished
    }
}

impl<R> Read for MailHeaderReader<R> where R: Read {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Error> {
        if buf.is_empty() {
            return Ok(0);
        }
        if self.is_unexpected_eof {
            return Err(Error::new(ErrorKind::UnexpectedEof, "EOF reached before end of headers"));
        }
        if self.is_finished {
            return Ok(0);
        }
        let mut processed_offset = 0;
        while buf.len() - processed_offset > 0 {
            while self.rd_buf_sz > 0 {
                let common_sz = min(self.rd_buf_sz as usize, buf.len() - processed_offset);
                buf[processed_offset..processed_offset + common_sz].clone_from_slice(&self.rd_buf[..common_sz]);
                self.rd_buf.rotate_left(common_sz);
                self.rd_buf_sz -= common_sz as u8;
                processed_offset += common_sz;
                continue;
            }
            let b = {
                let mut b = [0u8; 1];
                let sz = self.reader.read(&mut b)?;
                if sz == 0 {
                    if self.is_in_mail {
                        self.is_unexpected_eof = true;
                    } else {
                        self.is_finished = true;
                    }
                    // in fact if is not done an unexpected eof should be returned?
                    break;
                }
                b[0]
            };

            match self.bd.pass_byte(b) {
                BoundaryDetectorResult::NoMatch => {
                    buf[processed_offset] = b;
                    processed_offset += 1;
                    continue;
                }
                BoundaryDetectorResult::MatchBegin => {}
                BoundaryDetectorResult::MatchDone => {
                    self.is_finished = true;
                    break;
                }
                BoundaryDetectorResult::MatchBroke(v) => {
                    self.rd_buf[..v.len()].clone_from_slice(v);
                    self.rd_buf_sz = v.len() as u8;
                    self.rd_buf[self.rd_buf_sz as usize] = b;
                    self.rd_buf_sz += 1;
                    continue;
                }
            }
        }
        // returning Ok(0) is signal that it's non-error EOF
        // But in our case it may be error eof as well
        if processed_offset == 0 && self.is_unexpected_eof {
            return Err(Error::new(ErrorKind::UnexpectedEof, "EOF reached before end of headers"));
        }
        Ok(processed_offset)
    }
}

#[cfg(test)]
mod test {
    use std::io::{Cursor, sink};
    use std::io;

    use super::*;

    // TODO(teawithsand) perform test with different buffer sizes
    #[test]
    fn test_can_read_mail_headers() {
        for (i, o, iim) in [
            (
                "Subject: Hi\r\nX-Special-Header: ASDF\r\n\r\nSome mail body...",
                Some("Subject: Hi\r\nX-Special-Header: ASDF"),
                false,
            ),
            (
                "Subject: Hi\r\nX-Special-Header: ASDF\r\n\r\nSome mail body...",
                Some("Subject: Hi\r\nX-Special-Header: ASDF"),
                true,
            ),
            (
                "Subject: Hi\r\n\r\nSome mail body...",
                Some("Subject: Hi"),
                false,
            ),
            (
                "Subject: Hi\r\n\r\nSome mail body...",
                Some("Subject: Hi"),
                true,
            ),
            (
                "Subject: Hi",
                Some("Subject: Hi"),
                false,
            ),
            (
                "Subject: Hi",
                None,
                true,
            ),
        ].iter() {
            let mut r = Cursor::new(i.as_bytes());
            {
                let mut mr = MailHeaderReader::new(&mut r, *iim);
                if let Some(o) = o {
                    let mut buf = Vec::new();
                    mr.read_to_end(&mut buf).unwrap();
                    assert_eq!(&buf[..], o.as_bytes());
                } else {
                    io::copy(&mut mr, &mut sink()).unwrap_err();
                }
            }
        }
    }
}
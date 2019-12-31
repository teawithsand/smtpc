use std::io::{self, Cursor, Error, ErrorKind, Read, Write};
use std::num::ParseIntError;

use crate::utils::hex::encode_hex_char;

/// is_valid_hex_digit checks whatever or not given byte is valid ascii hex digit.
/// Because quoted printable should accept only uppercase hex digits this function accepts only uppercase data.
fn is_valid_hex_digit(num: u8) -> bool {
    match num {
        b'0'..=b'9' => true,
        b'A'..=b'F' => true,
        b'a'..=b'f' => true,
        _ => false,
    }
}

#[derive(Debug, From)]
pub enum QuotedPrintableDecodingError {
    ParseIntError(ParseIntError),
    InvalidEnd,
}

// TODO(teawithsand) rewrite to use streaming version on buffer
/// encode_quoted_printable encodes given string as quited printable 7 bit ascii
/// # Note
/// It does not limit char count in each line. It's caller responsibility to do so.
pub fn encode_quoted_printable<S: AsRef<[u8]>>(qp: S) -> String {
    let qp = qp.as_ref();
    let mut res = String::new();
    qp.iter()
        .copied()
        .for_each(|b| {
            // what about non printable chars?
            if b != b'=' && (b as char).is_ascii() {
                res.push(b as char);
            } else {
                res.push_str(&format!("={:X}", b))
            }
        });
    res
}


/// SoftLineBreaksMode describes if or how `QuotedPrintableWriter` should insert soft line breaks
/// due to line length limit
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SoftLineBreaksMode {
    /// Just skip it
    NoInsert,

    /// Insert `\r\n` line endings in order to satisfy line length 76 char limit.
    Standard,

    /// Insert `\n` line endings in order to satisfy line length 76 char limit.
    BreakLine,
}

impl Default for SoftLineBreaksMode {
    fn default() -> Self {
        SoftLineBreaksMode::Standard
    }
}

pub struct QuotedPrintableWriter<W> {
    writer: W,
    line_length: u8,

    line_break_mode: SoftLineBreaksMode,
}

impl<W> QuotedPrintableWriter<W> {

    pub fn new(writer: W, line_break_mode: SoftLineBreaksMode) -> Self {
        Self {
            writer,
            line_length: 0,

            line_break_mode,
        }
    }
}

impl<W> Write for QuotedPrintableWriter<W>
    where W: Write
{
    fn write(&mut self, mut buf: &[u8]) -> Result<usize, io::Error> {
        if buf.is_empty() {
            return Ok(0);
        }
        let original_buf_len = buf.len();
        loop {
            if buf.is_empty() {
                break;
            }

            let b = buf[0];
            if b.is_ascii() && b != b'\n' && b != b'\r' && b != b'=' {
                match self.line_break_mode {
                    SoftLineBreaksMode::NoInsert => {
                        self.line_length = 0;
                    }
                    SoftLineBreaksMode::Standard => {
                        if self.line_length + 1 >= 76 {
                            self.writer.write_all(b"=\r\n")?;
                            self.line_length = 0;
                        }
                    }
                    SoftLineBreaksMode::BreakLine => {
                        if self.line_length + 1 >= 76 {
                            self.writer.write_all(b"=\n")?;
                            self.line_length = 0;
                        }
                    }
                }
                self.line_length += 1;
                self.writer.write_all(&[b])?;
            } else {
                match self.line_break_mode {
                    SoftLineBreaksMode::NoInsert => {
                        self.line_length = 0;
                    }
                    SoftLineBreaksMode::Standard => {
                        if self.line_length + 3 >= 76 {
                            self.writer.write_all(b"=\r\n")?;
                            self.line_length = 0;
                        }
                    }
                    SoftLineBreaksMode::BreakLine => {
                        if self.line_length + 3 >= 76 {
                            self.writer.write_all(b"=\n")?;
                            self.line_length = 0;
                        }
                    }
                }
                self.line_length += 3;
                let enc = encode_hex_char(b);
                self.writer.write_all(&[
                    b'=', enc[0], enc[1]
                ])?;
            }
            buf = &buf[1..];
        }
        Ok(original_buf_len - buf.len())
    }

    fn flush(&mut self) -> Result<(), Error> {
        self.writer.flush()
    }
}

/// QuotedPrintableStream allows processing quoted printable data in stream manner.
pub struct QuotedPrintableReader<R> {
    stream: R,
    is_strict: bool,
    state: u8,
    buf: u8,
    is_error: bool,
}

impl<R> QuotedPrintableReader<R> {
    /// is_ok checks whatever or not is given quoted printable stream in
    /// 'done' state. This means that it is not in the middle of parsing some char like after consuming =F(F).
    /// It still has to wait for second F in order to given proper response
    pub fn is_ok(&self) -> bool {
        !self.is_error
    }

    pub fn new(s: R) -> Self {
        Self {
            stream: s,
            buf: 0,
            state: 0,
            is_error: false,
            is_strict: false,
        }
    }
}

impl<R> Read for QuotedPrintableReader<R>
    where R: Read
{
    fn read(&mut self, mut buf: &mut [u8]) -> Result<usize, Error> {
        if self.is_error {
            return Err(io::Error::new(ErrorKind::InvalidData, "Got invalid character after '=' char"));
        }
        if buf.is_empty() {
            return Ok(0);
        }
        let mut original_buf_len = buf.len();
        loop {
            if buf.is_empty() {
                break; // we ran out of space to write to
            }
            let b = {
                let mut arr = [0u8; 1];
                let len = self.stream.read(&mut arr)?;
                if len == 0 {
                    break;
                }
                arr[0]
            };
            if self.state == 0 {
                if self.is_strict && !b.is_ascii() {
                    self.is_error = true;
                    break;
                }
                if b == b'=' {
                    self.state = 1;
                } else {
                    buf[0] = b;
                    buf = &mut buf[1..];
                }
            } else if self.state == 1 {
                // soft line break
                if b == b'\n' {
                    self.state = 0;
                } else if b == b'\r' { // maybe it's begin of soft line break
                    self.state = 3;
                } else {
                    if !is_valid_hex_digit(b) {
                        self.is_error = true;
                        break;
                    }
                    self.buf = b;
                    self.state = 2;
                }
            } else if self.state == 2 {
                if !is_valid_hex_digit(b) {
                    self.is_error = true;
                    break;
                }
                let mut res = 0;
                match self.buf.to_ascii_uppercase() {
                    b @ b'0'..=b'9' => {
                        res += (b - b'0') * 16;
                    }
                    b @ b'A'..=b'F' => {
                        res += (b - b'A' + 10) * 16;
                    }
                    _ => unreachable!("Invalid ascii char"),
                }
                match b.to_ascii_uppercase() {
                    b @ b'0'..=b'9' => {
                        res += b - b'0';
                    }
                    b @ b'A'..=b'F' => {
                        res += b - b'A' + 10;
                    }
                    _ => unreachable!("Invalid ascii char"),
                }
                buf[0] = res;
                buf = &mut buf[1..];
                self.state = 0;
            } else if self.state == 3 {
                if b == b'\n' {
                    // yep just soft line break
                } else if self.is_strict {
                    self.is_error = true;
                    break;
                }
                self.state = 0;
            } else {
                unreachable!("Invalid state!");
            }
        }
        if self.is_error {
            return Err(io::Error::new(ErrorKind::InvalidData, "Got invalid character after '=' char"));
        }
        Ok(original_buf_len - buf.len())
    }
}

#[cfg(test)]
mod test {
    use std::io::Cursor;

    use super::*;

    fn stream_parse(data: &[u8]) -> Vec<u8> {
        let mut c = Cursor::new(data
            .iter()
            .copied()
            .collect::<Vec<_>>()
        );
        let mut qpr = QuotedPrintableReader::new(&mut c);
        let mut res = Vec::new();
        qpr.read_to_end(&mut res).unwrap();
        assert!(qpr.is_ok());
        res
    }

    #[test]
    fn test_can_encode_quoted_printable() {
        for (input, output) in [
            ("ŁĄŻŹ", "=C5=81=C4=84=C5=BB=C5=B9"),
            ("aaŁŁ", "aa=C5=81=C5=81"),
            ("=", "=3D"),
            (
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                // line with = char len is eq to 76 which is standard limit
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa=\r\na"
            ),
            (
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaŁ",
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa=\r\n=C5=81"
            ),
            (
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaŁa",
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa=\r\n=C5=81a"
            ),
        ].iter() {
            let mut w = Cursor::new(Vec::new());
            let mut qpw = QuotedPrintableWriter::new(&mut w, SoftLineBreaksMode::default());
            qpw.write_all(input.as_bytes()).unwrap();
            let given_output = String::from_utf8(w.into_inner()).unwrap();
            //assert_eq!(output, &encode_quoted_printable(input));
            assert_eq!(output, &given_output);
        }
    }

    //noinspection SpellCheckingInspection
    #[test]
    fn test_can_decode_quoted_printable() {
        for (input, output) in [
            ("asdf", "asdf"),
            ("aa=C5=81=C5=81", "aaŁŁ"),
            ("aa\naa", "aa\naa"),
            ("aa\r\naa", "aa\r\naa"),
            ("aa=\r\naa", "aaaa"),
            ("aa=\naa", "aaaa"),
            (
                // according to standard line limit is 76 chars
                // = at the end is used as soft line break
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa=\na",
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
            ),
            (
                // according to standard line limit is 76 chars
                // = at the end is used as soft line break
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa=\r\na",
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
            ),
            (
                // according to standard line limit is 76 chars
                // = at the end is used as soft line break
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa=\r\n=C5=81",
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaŁ"
            ),
            (
                &"=61".repeat(25),
                &"a".repeat(25)
            ),
            (
                &format!("{}=\n", "=61".repeat(25)),
                &"a".repeat(25)
            ),
            (
                &format!("{}=\n", "=61".repeat(25)),
                &"a".repeat(25)
            ),
            // non-standard tests(line len > 76 chars)
            (
                &"=61".repeat(50),
                &"a".repeat(50)
            ),
            (
                &format!("{}=\n", "=61".repeat(50)),
                &"a".repeat(50)
            ),
        ].iter() {
            // eprintln!("Testing: {:?}", input);
            // assert_eq!(output, &String::from_utf8(decode_quoted_printable(input).unwrap()).unwrap());
            let res = stream_parse(input.as_bytes());
            assert_eq!(&String::from_utf8(res).unwrap(), output);
        }
    }
}
use std::io::{self, Cursor, Error, ErrorKind, Read, Write};
use std::num::ParseIntError;

use crate::encoding::{Decoder, Encoder};
use crate::utils::hex::encode_hex_char;

/// is_valid_hex_digit checks whatever or not given byte is valid ascii hex digit.
/// Because quoted printable should accept only uppercase hex digits this function accepts only uppercase data.
fn is_valid_hex_digit(num: u8) -> bool {
    match num {
        b'0'..=b'9' => true,
        b'A'..=b'F' => true,
        _ => false,
    }
}

#[derive(Debug, From)]
pub enum QuotedPrintableDecodingError {
    ParseIntError(ParseIntError),
    InvalidEnd,
}

// TODO(teawithsand) rewrite to use streamming version on buffer
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

pub struct QuotedPrintableWriter<W> {
    writer: W,
    write_buf: [u8; 5],
    write_buf_sz: u8,
    line_len: u8,

    last_written_char: u8,
}

pub struct QuotedPrintableEncoder {}

impl Encoder for QuotedPrintableEncoder {
    fn encode_to_string(input: &[u8], res: &mut String) -> usize {
        let ov = Vec::with_capacity(input.len());
        let mut w = Cursor::new(ov);
        {
            let mut qpw = QuotedPrintableWriter::new(&mut w);
            qpw.write_all(input).expect("Writing to cursor should never fail");
        }
        let new_res = String::from_utf8(w.into_inner()).expect("Quoted printable text is not valid utf8");
        let new_res_sz = new_res.len();
        if res.is_empty() {
            *res = new_res;
        } else {
            res.push_str(
                &new_res
            );
        }

        new_res_sz
    }
}

impl<W> QuotedPrintableWriter<W> {
    /*
    pub fn encode(text: &[u8]) -> String {
        let mut ov = Vec::with_capacity(text.len());
        let mut w = Cursor::new(ov);
        {
            let mut qpw = QuotedPrintableWriter::new(&mut w);
            qpw.write_all(text).expect("Writing to cursor should never fail");
        }
        String::from_utf8(w.into_inner()).expect("Quoted printable text is not valid utf8")
    }
    */

    pub fn new(writer: W) -> Self {
        Self {
            writer,
            write_buf: [0u8; 5],
            write_buf_sz: 0,

            line_len: 0,
            last_written_char: 0,
        }
    }
}

impl<W> QuotedPrintableWriter<W>
    where W: Write
{
    fn flush_write_buf(&mut self) -> Result<(), io::Error> {
        while self.write_buf_sz > 0 {
            let len = self.writer.write(&self.write_buf[..self.write_buf_sz as usize]).unwrap();
            if len == 0 {
                return Err(Error::new(ErrorKind::WriteZero, "Write of len zero occurred to dst buffer"));
            }
            debug_assert!(len <= self.write_buf_sz as usize);
            self.write_buf_sz -= len as u8;
        }
        Ok(())
    }
}

impl<W> Write for QuotedPrintableWriter<W>
    where W: Write
{
    fn write(&mut self, buf: &[u8]) -> Result<usize, io::Error> {
        if buf.is_empty() {
            return Ok(0);
        }
        let mut written_sz = 0;
        for b in buf.iter().cloned() {
            self.flush_write_buf()?;
            let (enc_char, enc_char_sz) = match b {
                b if b.is_ascii() && b != b'=' => {
                    ([b, 0, 0], 1)
                }
                b => {
                    let d = encode_hex_char(b);
                    ([b'=', d[0], d[1]], 3)
                }
            };
            debug_assert!(enc_char_sz == 1 || enc_char_sz == 3);
            if b == b'\n' {
                self.line_len = 0;
            } else {
                self.line_len += enc_char_sz;
            }
            if self.line_len + enc_char_sz > 76 {
                // emit break line first unless it's not needed
                if self.last_written_char != b'\n' {
                    self.write_buf[self.write_buf_sz as usize] = b'=';
                    self.write_buf_sz += 1;
                }
                self.write_buf[self.write_buf_sz as usize] = b'\n';
                self.write_buf_sz += 1;

                self.line_len = 0;
            }
            self.write_buf[self.write_buf_sz as usize..self.write_buf_sz as usize + enc_char_sz as usize]
                .clone_from_slice(&enc_char[..enc_char_sz as usize]);
            self.write_buf_sz += enc_char_sz as u8;

            debug_assert!(self.write_buf_sz <= 5);
            debug_assert!(self.write_buf_sz >= 1);

            self.last_written_char = b;
            written_sz += 1;
        }
        self.flush_write_buf()?;
        eprintln!("In sz: {} written sz: {}", buf.len(), written_sz);

        Ok(written_sz)
    }

    fn flush(&mut self) -> Result<(), io::Error> {
        self.flush_write_buf()?;
        self.writer.flush()
    }
}

// TODO(teawithsand) rewrite to use streaming version on buffer
pub fn decode_quoted_printable<S: AsRef<str>>(qp: S) -> Result<Vec<u8>, QuotedPrintableDecodingError> {
    let qp = qp.as_ref();
    let mut res = Vec::new();

    let mut first_hex_char: char = 'a';
    let mut state = 0;

    let mut is_first_line = true;
    let mut is_soft_break = false;
    let mut is_cr_new_line = false;

    for l in qp.split_terminator('\n') {
        // add \n from prev line.
        // This is used to handle last line properly(no \n at end)

        if !is_soft_break && !is_first_line {
            if is_cr_new_line {
                res.extend_from_slice(b"\r\n");
            } else {
                res.push(b'\n');
            }
        }

        is_first_line = false;

        is_soft_break = false;
        let mut l = l;
        let trimmed_line = l.trim_end_matches(|c| c == '\r');
        if trimmed_line.ends_with('=') {
            let mut len_diff = l.len() - trimmed_line.len();
            if len_diff > 0 {
                // panic!("GOT \r eol!");
                len_diff = 1;
                is_cr_new_line = true;
            } else {
                is_cr_new_line = false;
            }

            l = &l[..l.len() - 1 - len_diff]; // '=' char has one byte
            is_soft_break = true;
        }

        for c in l.chars() {
            if state == 0 {
                if c == '=' {
                    state = 1;
                } else {
                    // we don't check whatever or not is char ascii char but this seems to be ok
                    let mut buf = [0u8; 4];
                    let str_res = c.encode_utf8(&mut buf);
                    res.extend_from_slice(&str_res.as_bytes()[..str_res.len()]);
                }
            } else if state == 1 {
                first_hex_char = c;
                state = 2;
            } else if state == 2 {
                let mut hex_data = String::new();
                hex_data.push(first_hex_char);
                hex_data.push(c);
                let byte = u8::from_str_radix(&hex_data, 16)?;
                res.push(byte);
                state = 0;
            }
        }
    }

    Ok(res)
}

/// QuotedPrintableStream allows processing quoted printable data in stream manner.
pub struct QuotedPrintableReader<R> {
    stream: R,
    buf: u8,
    buf_offset: u8,
    is_error: bool,
}

pub struct QuotedPrintableDecoder {}

impl Decoder for QuotedPrintableDecoder {
    type Error = io::Error;

    fn decode_to_string(input: &[u8], res: &mut String) -> Result<usize, Self::Error> {
        let mut c = Cursor::new(input);
        {
            let mut qpr = QuotedPrintableReader::new(&mut c);
            Ok(qpr.read_to_string(res)?)
        }
    }
}

impl<R> QuotedPrintableReader<R> {
    /// is_ok checks whatever or not is given quoted printable stream in
    /// 'done' state. This means that it is not in the middle of parsing some char like after consuming =F(F).
    /// It still has to wait for second F in order to given proper response
    pub fn is_ok(&self) -> bool {
        !self.is_error && self.buf_offset == 0
    }

    pub fn new(s: R) -> Self {
        Self {
            stream: s,
            buf: 0,
            buf_offset: 0,
            is_error: false,
        }
    }
}

impl<R> Read for QuotedPrintableReader<R>
    where R: Read
{
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, io::Error> {
        if self.is_error {
            return Err(io::Error::new(ErrorKind::InvalidData, "Got invalid character after '=' char"));
        }

        if buf.is_empty() {
            return Ok(0);
        }

        let mut local_buf = vec![0; buf.len()];
        // TODO(teawithsand) fix case when data is read to buffer and error occurrs and state is corrupted now.
        //  either store it in to load buffer or cache error and return it later on next read try.
        let len = self.stream.read(&mut local_buf)?;
        let local_buf = &local_buf[..len];

        // input encoding has to be ascii (this is why quoted printable exists)
        // so input is byte per char.
        // Two byte buffer is needed because we might read =xx

        // TODO(teawithsand) remove that vector away. It's here because we don't want to modify buf if error occurred.
        let mut res_buf = Vec::with_capacity(buf.len());

        let mut out_buf_written_offset = 0;
        for b in local_buf.iter().copied() {
            debug_assert_eq!(res_buf.len(), out_buf_written_offset);
            if out_buf_written_offset >= buf.len() {
                break;
            }

            if self.buf_offset == 0 {
                if b == b'=' {
                    self.buf_offset = 1;
                } else {
                    //buf[out_buf_written_offset] = b;
                    res_buf.push(b);
                    out_buf_written_offset += 1;
                }
            } else if self.buf_offset == 1 {
                if b == b'\n' {
                    // = sign used as soft new line.
                    // ignore it
                    // note: this may not be 100% standard behaviour but seems to be ok
                    self.buf_offset = 0;
                    continue;
                }
                if b == b'\r' {
                    // same as above but there should be \n char after it
                    self.buf_offset = 3;
                    continue;
                }
                if !is_valid_hex_digit(b) {
                    self.is_error = true;
                    return Err(io::Error::new(ErrorKind::InvalidData, "Got invalid character after '=' char"));
                }
                self.buf = b;
                self.buf_offset = 2;
            } else if self.buf_offset == 2 {
                if !is_valid_hex_digit(b) {
                    self.is_error = true;
                    return Err(io::Error::new(ErrorKind::InvalidData, "Got invalid character after '=' char"));
                }
                // this has to succeed due to above validations
                let b = u8::from_str_radix(&format!("{}{}", self.buf as char, b as char), 16).unwrap();
                //buf[out_buf_written_offset] = b;
                res_buf.push(b);
                out_buf_written_offset += 1;

                self.buf_offset = 0;
            } else if self.buf_offset == 3 {
                if b != b'\n' {
                    self.is_error = true;
                    return Err(io::Error::new(ErrorKind::InvalidData, "Got invalid character after '=' char"));
                }
                self.buf_offset = 0;
            } else {
                panic!("Should never happen")
            }

            /*
                else if self.buf_offset == 4 {
                    if b == b'=' {
                        //buf[out_buf_written_offset] = b;
                        res_buf.push(b'\r');
                        out_buf_written_offset += 1;
                        self.buf_offset = 1;
                    }
                    if b == b'\n' {
                        //buf[out_buf_written_offset] = b;
                        res_buf.push(b'\n');
                        out_buf_written_offset += 1;
                    } else {
                        // This might write out of the buffer....(adds 2 in 1 iter)...
                        //buf[out_buf_written_offset] = b;
                        res_buf.push(b'\r');
                        out_buf_written_offset += 1;
                        //buf[out_buf_written_offset] = b;
                        res_buf.push(b);
                        out_buf_written_offset += 1;
                    }
                    self.buf_offset = 0;
                }
            */
        }

        debug_assert!(res_buf.len() <= buf.len());
        res_buf.iter()
            .copied()
            .enumerate()
            .for_each(|(i, b)| buf[i] = b);
        Ok(out_buf_written_offset)
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
        let mut qpd = QuotedPrintableReader::new(&mut c);
        let mut res = Vec::new();
        qpd.read_to_end(&mut res).unwrap();
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
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa=\na"
            ),
            (
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaŁ",
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa=\n=C5=81"
            ),
            (
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaŁa",
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa=\n=C5=81a"
            ),
        ].iter() {
            let mut w = Cursor::new(Vec::new());
            let mut qpw = QuotedPrintableWriter::new(&mut w);
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
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa=\na",
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
            ),
        ].iter() {
            assert_eq!(output, &String::from_utf8(decode_quoted_printable(input).unwrap()).unwrap());
            let res = stream_parse(input.as_bytes());
            assert_eq!(&String::from_utf8(res).unwrap(), output);
        }
    }
}
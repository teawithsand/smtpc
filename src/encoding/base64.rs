use std::cmp::min;
use std::io::{self, Error, ErrorKind, Read, Write};

use crate::encoding::{Decoder, Encoder};

pub struct Base64Reader<R> {
    reader: R,

    // buf contains output data which was read in order to read multiply of 4 to decode to valid base64
    in_buf: [u8; 4],
    in_buf_sz: u8,

    rd_buf: [u8; 3],
    rd_buf_sz: u8,

    padding_cnt: u8,
    is_err: bool,
}

pub struct Base64Decoder {}

impl Decoder for Base64Decoder {
    type Error = io::Error;

    fn decode_to_string(input: &[u8], res: &mut String) -> Result<usize, Self::Error> {
        let new_res = base64::decode(input)
            .map_err(|_| io::Error::new(ErrorKind::Other, "Invalid base64 input"))?;
        let new_res_str = String::from_utf8(new_res)
            .map_err(|_| io::Error::new(ErrorKind::Other, "Given decoded base64 data is not valid utf8 string"))?;
        let sz = new_res_str.len();
        if res.len() == 0 {
            *res = new_res_str;
        } else {
            res.push_str(&new_res_str);
        }
        Ok(sz)
    }
}

impl<R> Base64Reader<R> {
    #[inline]
    pub fn new(reader: R) -> Base64Reader<R> {
        Base64Reader {
            reader,
            in_buf: [0u8; 4],
            in_buf_sz: 0,

            rd_buf: [0u8; 3],
            rd_buf_sz: 0,

            padding_cnt: 0,
            is_err: false,
        }
    }
}

impl<R: Read> Read for Base64Reader<R> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, io::Error> {
        if buf.is_empty() {
            return Ok(0);
        }

        let mut buffer_offset = 0;
        while buf.len() - buffer_offset > 0 {
            if self.rd_buf_sz > 0 {
                // eprintln!("There's data in rd buf len: {} data: {:?}", self.rd_buf_sz, &self.rd_buf);
                let common_sz = min(buf.len() - buffer_offset, self.rd_buf_sz as usize);
                debug_assert!(common_sz > 0);
                (&mut buf[buffer_offset..buffer_offset + common_sz]).clone_from_slice(&self.rd_buf[..common_sz]);
                buffer_offset += common_sz;

                // TODO(teawithsand) more efficient version: memmove instead of rotate
                self.rd_buf.rotate_left(common_sz);
                self.rd_buf_sz -= common_sz as u8;

                continue;
            }

            // TODO(teawithsand) rather than byte per byte read populate 4 byte array in self
            // TODO(teawithsand) then optimize that so it's able to read multiple of 4 bytes and pass them into reader
            let b = {
                let mut b = [0u8; 1];
                // TODO(teawithsand) fix case when data is read to buffer and error occurrs and state is corrupted now.
                //  either store it in to load buffer or cache error and return it later on next read try.
                let done_read_sz = self.reader.read(&mut b)?;
                if done_read_sz == 0 {
                    // eprintln!("Done! Got zero read. In buf sz: {} Rd buf sz: {}", self.in_buf_sz, self.rd_buf_sz);
                    if self.in_buf_sz != 0 {
                        return Err(Error::new(ErrorKind::UnexpectedEof, "Base64 stream ended unexpectedly"));
                    }
                    break;
                }
                b[0]
            };

            if b == b'=' {
                if self.padding_cnt >= 2 {
                    self.is_err = true;
                    return Err(Error::new(ErrorKind::Other, "Base64 data invalid. Found too much padding"));
                }
                self.padding_cnt += 1;
            }
            if self.padding_cnt > 0 && b != b'=' {
                self.is_err = true;
                return Err(Error::new(ErrorKind::Other, "Base64 data invalid. Found non padding char after padding one"));
            }

            if self.in_buf_sz < 3 {
                // eprintln!("Inc in buf sz: {} to {}", self.in_buf_sz, self.in_buf_sz + 1);
                self.in_buf[self.in_buf_sz as usize] = b;
                self.in_buf_sz += 1;
            } else if self.in_buf_sz == 3 {
                self.in_buf[3] = b;
                self.in_buf_sz = 0;
                // eprintln!("Decoding value: {:?}", std::str::from_utf8(&self.in_buf));

                if buf[buffer_offset..].len() >= 3 {
                    // once error occurred no more read should occur?
                    let len = base64::decode_config_slice(&self.in_buf, base64::STANDARD, &mut buf[buffer_offset..])
                        .map_err(|err| io::Error::new(
                            ErrorKind::Other,
                            format!("Base64 decode error: {}", err.to_string()),
                        ))?;
                    buffer_offset += len;
                    continue;
                } else {
                    // eprintln!("No space in output. Decoding into rd buff. value: {:?}", std::str::from_utf8(&self.in_buf));
                    let len = base64::decode_config_slice(&self.in_buf, base64::STANDARD, &mut self.rd_buf)
                        .map_err(|err| io::Error::new(
                            ErrorKind::Other,
                            format!("Base64 decode error: {}", err.to_string()),
                        ))?;
                    debug_assert!(len <= 3);
                    self.rd_buf_sz = len as u8;
                    // eprintln!("RD buf sz: {}", self.rd_buf_sz);
                    continue;
                }
            } else {
                unreachable!();
            }
        };

        Ok(buffer_offset)
    }
}

pub struct Base64Writer<W> {
    writer: W,

    in_buf: [u8; 3],
    in_buf_sz: u8,

    w_buf: [u8; 4],
    w_buf_sz: u8,

    is_done: bool,
    finalize_on_flush: bool,
}

pub struct Base64Encoder {}

impl Encoder for Base64Encoder {
    fn encode_to_string(input: &[u8], res: &mut String) -> usize {
        let new_res = base64::encode(input);
        let new_res_sz = new_res.len();
        if res.is_empty() {
            *res = new_res;
        } else {
            res.push_str(&new_res);
        }
        new_res_sz
    }
}

impl<W> Base64Writer<W> {
    pub fn new(writer: W, finalize_on_flush: bool) -> Self {
        Self {
            writer,
            in_buf: [0u8; 3],
            in_buf_sz: 0,

            w_buf: [0u8; 4],
            w_buf_sz: 0,

            is_done: false,
            finalize_on_flush,
        }
    }

    pub fn is_finalized(&self) -> bool {
        self.is_done
    }

    fn check_finalized(&self) -> Result<(), io::Error> {
        if self.is_done {
            Err(io::Error::new(ErrorKind::Other, "Can't write any more data. Writer was finalized."))
        } else {
            Ok(())
        }
    }
}

impl<W> Base64Writer<W> where W: Write {
    fn flush_write_buffer(&mut self) -> Result<(), io::Error> {
        while self.w_buf_sz > 0 {
            match self.writer.write(&self.w_buf[..self.w_buf_sz as usize]) {
                Ok(0) => {
                    return Err(io::Error::new(ErrorKind::WriteZero, "Flush buffers field to write data. Ok(0) was returned."));
                }
                Ok(v) => {
                    debug_assert!(v <= self.w_buf_sz as usize);
                    self.w_buf_sz -= v as u8;
                }
                Err(e) => match e.kind() {
                    // ignore interrupted error kind?
                    ErrorKind::Interrupted => {}
                    _ => {
                        return Err(e);
                    }
                }
            }
        }
        Ok(())
    }

    fn read_buffer_into_write_buffer(&mut self) {
        debug_assert_eq!(self.w_buf_sz, 0);

        let len = base64::encode_config_slice(
            &self.in_buf[..self.in_buf_sz as usize],
            base64::STANDARD,
            &mut self.w_buf,
        );
        if self.in_buf_sz > 0 {
            debug_assert_eq!(len, 4);
        } else {
            debug_assert_eq!(len, 0);
        }
        self.in_buf_sz = 0;
        self.w_buf_sz = len as u8;
    }

    fn flush_buffers(&mut self) -> Result<(), io::Error> {
        self.flush_write_buffer()?;
        self.read_buffer_into_write_buffer();
        self.flush_write_buffer()?;
        debug_assert_eq!(self.w_buf_sz, 0);
        debug_assert_eq!(self.in_buf_sz, 0);
        Ok(())
    }

    pub fn finalize(&mut self) -> Result<(), io::Error> {
        self.check_finalized()?;
        self.flush_buffers()?;
        self.is_done = true;

        Ok(())
    }
}
// impl drop and log warning once not finalized and dropped? maybe panic?

impl<W> Write for Base64Writer<W> where W: Write {
    #[allow(clippy::cognitive_complexity)]
    fn write(&mut self, buf: &[u8]) -> Result<usize, Error> {
        self.check_finalized()?;
        if buf.is_empty() {
            return Ok(0);
        }
        let mut processed_sz = 0;

        // TODO(teawithsand) optimize so can serialize more than 4 bytes at a time through base64::encode
        while buf.len() - processed_sz > 0 {
            if self.w_buf_sz > 0 {
                let len = self.writer.write(&self.w_buf[..self.w_buf_sz as usize])?;
                debug_assert!(len <= self.w_buf_sz as usize);
                self.w_buf_sz -= len as u8;
                continue;
            }
            if self.in_buf_sz > 0 && self.in_buf_sz < 3 {
                let max_common_sz = min(buf[processed_sz..].len(), self.in_buf.len() - self.in_buf_sz as usize);
                self.in_buf[self.in_buf_sz as usize..self.in_buf_sz as usize + max_common_sz].clone_from_slice(&buf[..max_common_sz]);
                self.in_buf_sz += max_common_sz as u8;
                debug_assert!(self.in_buf_sz <= 3);
                // eprintln!("Put data into in_buf_sz: amount now: {} amount: {}", self.in_buf_sz, max_common_sz);

                processed_sz += max_common_sz;
                // eprintln!("Processed sz now: {} Buf sz: {}", processed_sz, buf.len());
                continue;
            }

            if self.in_buf_sz == 3 {
                let len = base64::encode_config_slice(
                    &self.in_buf,
                    base64::STANDARD,
                    &mut self.w_buf,
                );
                debug_assert_eq!(len, 4);
                self.w_buf_sz = len as u8;

                let len = self.writer.write(&self.w_buf[..self.w_buf_sz as usize])?;
                if len == 0 {
                    return Err(Error::new(ErrorKind::WriteZero, "Write returned Ok(0)"));
                }

                debug_assert!(len <= 4);
                self.w_buf_sz -= len as u8;

                self.in_buf_sz = 0;
                continue;
            }
            debug_assert_eq!(self.in_buf_sz, 0);

            if processed_sz + 3 <= buf.len() {
                let rd_buf = &buf[processed_sz..processed_sz + 3];
                debug_assert_eq!(rd_buf.len(), 3);

                let len = base64::encode_config_slice(
                    &rd_buf[..],
                    base64::STANDARD,
                    &mut self.w_buf,
                );
                debug_assert_eq!(len, 4);
                self.w_buf_sz = len as u8;

                let len = self.writer.write(&self.w_buf[..self.w_buf_sz as usize])?;
                debug_assert!(len <= 4);
                self.w_buf_sz -= len as u8;

                processed_sz += 3;

                continue;
            } else {
                let rd_buf = &buf[processed_sz..];
                debug_assert!(rd_buf.len() < 3);
                debug_assert_eq!(self.in_buf_sz, 0);
                self.in_buf[..rd_buf.len()].clone_from_slice(&rd_buf);
                // eprintln!("Put {} bytes into in-buf", rd_buf.len());
                self.in_buf_sz = rd_buf.len() as u8;
                processed_sz += rd_buf.len();

                continue;
            }
        }

        Ok(processed_sz)
    }

    fn flush(&mut self) -> Result<(), Error> {
        if self.is_finalized() {
            self.writer.flush()?;
            return Ok(());
        }

        if self.finalize_on_flush {
            // hack warning: this prevents infinite recursion
            self.finalize_on_flush = false;
            let res = self.finalize();
            self.finalize_on_flush = true; // it may fail. To allow proper behaviour reinitialize this variable.
            res?;
        } else {
            self.flush_buffers()?;
        }
        self.writer.flush()
    }
}

#[cfg(test)]
mod test {
    use std::io::Cursor;

    use super::*;

    fn perform_test_decode_gives_same_result(data: &str, buf_sz: usize) {
        // eprintln!("Testing {:?}", data);
        let res = base64::decode(data);
        let c = Cursor::new(data.as_bytes());
        let mut b64s = Base64Reader::new(c);
        let mut d = Vec::new();
        let st_res = loop {
            let mut rd_buf = vec![0u8; buf_sz];
            let len = match b64s.read(&mut rd_buf) {
                Ok(v) => v,
                Err(e) => {
                    break Err(e);
                }
            };
            if len == 0 {
                break Ok(());
            }
            d.extend_from_slice(&rd_buf[..len]);
        };
        if let Ok(res) = res {
            st_res.unwrap();
            assert_eq!(res, d);
        } else {
            assert!(st_res.is_err());
        }
    }

    fn perform_test_encode_gives_same_result(data: &[u8], buf_sz: usize) {
        assert!(buf_sz > 0);
        // eprintln!("Testing: {:?}", std::str::from_utf8(data));

        let res = base64::encode(data);
        let mut w_sink = Cursor::new(Vec::new());
        {
            let mut w = Base64Writer::new(&mut w_sink, false);
            let mut data = data;
            loop {
                let buf_sz = min(data.len(), buf_sz);
                if buf_sz == 0 {
                    break;
                }
                match w.write(&data[..buf_sz]) {
                    Ok(v) => {
                        debug_assert_ne!(v, 0);
                        // eprintln!("Written {} bytes", v);
                        data = &data[v..];
                    }
                    Err(e) => {
                        panic!("Got error: {:?}", e)
                    }
                };
            }
            w.finalize().unwrap();
            w.flush().unwrap();
            if !data.is_empty() {
                panic!("Entire data was not written. Last {} bytes", data.len());
            }
        }

        let sink = w_sink.into_inner();
        assert_eq!(String::from_utf8(sink).unwrap(), res);
    }

    #[test]
    fn test_decode_gives_same_result() {
        for d in [
            "garbage",
            "",
            "YQ==",
            "YWE=",
            "YWFh",
            "YWFhYQ==",
            "YWFhYWE=",
            "YWFhYWFh",
            "YWFhYWFhYQ==",
            "YWFhYWFhYWE=",
            "YWFhYWFhYWFh",
            "YWFhYWFhYWFhYQ==",
            "YWFhYWFhYWFhYWE=",
            "YWFhYWFhYWFhYWFh",
            "WYQ=YQ==",
        ].iter() {
            for buf_sz in [1, 2, 3, 4, 5, 8, 16, 32, 64, 128, 256].iter().cloned() {
                perform_test_decode_gives_same_result(d, buf_sz);
            }
        }
    }

    #[test]
    fn test_encode_gives_same_result() {
        for d in [
            b"" as &'static [u8],
            b"a",
            b"aa",
            b"aaa",
            b"aaaa",
            b"aaaaa",
            b"aaaaaa",
        ].iter().map(|d| *d) {
            for buf_sz in [1, 2, 3, 4, 5, 8, 16, 32, 64, 128, 256].iter().cloned() {
                perform_test_encode_gives_same_result(d, buf_sz);
            }
        }
    }
}
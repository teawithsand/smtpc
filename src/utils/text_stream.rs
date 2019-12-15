use std::io;
use std::io::{Error, Read};

#[derive(Debug, From)]
pub enum LineReadResult {
    IOError(io::Error),
    ZeroSizeRead,
    LineFound,
    BufferTooSmall,
}

pub trait TextStreamExt {
    /// read_until_crlf reads new line until it finds `\r\n` sequence.
    /// \r\n sequence is present at the end of line
    ///
    /// Second param of ok is whatever or not has overflow occurred. If it's true
    /// The line is longer than incoming buffer allows for.
    ///
    /// It reads input byte by byte and should be used only on buffered streams.
    fn read_until_crlf(&mut self, buf: &mut [u8]) -> Result<(usize, LineReadResult), io::Error>;
}

impl<T> TextStreamExt for T where T: Read {
    // TODO(teawithsand) handle error case - when too much data has been read
    fn read_until_crlf(&mut self, buf: &mut [u8]) -> Result<(usize, LineReadResult), io::Error> {
        let mut last_sz = buf.len();
        let mut state = 0;
        loop {
            if last_sz == 0 {
                return Ok((buf.len(), LineReadResult::BufferTooSmall));
            }
            let b = {
                let mut buf = [0u8; 1];
                let sz = match self.read(&mut buf) {
                    Ok(v) => v,
                    Err(e) => {
                        // if written any data to buffer already then do not return error
                        // pretend success
                        if last_sz != buf.len() {
                            return Ok((last_sz, LineReadResult::IOError(e)));
                        } else {
                            return Err(e);
                        }
                    }
                };
                if sz == 0 {
                    // yep... we can't do much about that
                    if last_sz != buf.len() {
                        return Ok((buf.len() - last_sz, LineReadResult::ZeroSizeRead));
                    } else {
                        return Ok((0, LineReadResult::ZeroSizeRead));
                    }
                }
                buf[0]
            };
            buf[buf.len() - last_sz] = b;
            last_sz -= 1;
            if state == 0 && b == b'\n' {
                state = 1;
            } else if state == 1 && b == b'\r' {
                return Ok((buf.len() - last_sz, LineReadResult::LineFound));
            } else if state == 1 { // b != b'\r' here
                state = 0;
            }
        }
    }
}
// TODO(teawithsand) some tests for stream read ext
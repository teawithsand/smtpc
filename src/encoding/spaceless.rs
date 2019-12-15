//! spaceless defines reader which is capable of throwing away white chars
//! It can be used in order to seamlessly decode base64 from messages as base64 reader does not tolerate newlines

use std::io::{Error, Read};

pub struct SpacelessReader<R> {
    reader: R
}

impl<R> SpacelessReader<R> {
    pub fn new(reader: R) -> Self {
        Self {
            reader,
        }
    }
}

impl<R> Read for SpacelessReader<R>
    where R: Read
{
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Error> {
        if buf.is_empty() {
            return Ok(0);
        }
        let mut processed_sz = 0;
        while buf.len() - processed_sz > 0 {
            let b = {
                let mut b = [0u8; 1];
                let sz = self.reader.read(&mut b)?;
                if sz == 0 {
                    break;
                }
                b[0]
            };
            if b == b'\n' || b == b'\r' || b == b' ' || b == b'\t' {
                continue;
            }
            buf[processed_sz] = b;
            processed_sz += 1;
        }
        Ok(processed_sz)
    }
}

// TODO(teawithsand) test it
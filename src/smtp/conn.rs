use std::io::{ErrorKind, Read, Write};
use std::io;
use std::string::FromUtf8Error;

use crate::utils::text_stream::{LineReadResult, TextStreamExt};

pub struct SMTPConn<S> {
    stream: S
}

impl<S> SMTPConn<S> {
    pub fn new(stream: S) -> Self {
        Self {
            stream,
        }
    }
}

pub type SMTPCode = u16;

pub fn is_valid_smtp_code(code: u16) -> bool {
    code >= 100 && code <= 999
}

#[derive(Debug, From)]
pub enum SMTPConnError {
    IOError(std::io::Error),
    FromUtf8Error(FromUtf8Error),
}

impl<S> SMTPConn<S> where S: Read + Write {
    /// server_read_command reads command from SMTP stream when this connection acts as a server.
    /// It returns size of data read and true if `\r\n`. False otherwise.
    pub fn server_read_command(&mut self, buf: &mut [u8]) -> Result<(usize, bool), SMTPConnError> {
        match self.stream.read_until_crlf(buf) {
            Ok((sz, LineReadResult::IOError(_))) | Ok((sz, LineReadResult::BufferTooSmall)) => {
                Ok((sz, false))
            }
            Ok((sz, LineReadResult::ZeroSizeRead)) => {
                if sz == 0 {
                    Err(SMTPConnError::IOError(
                        io::Error::new(ErrorKind::UnexpectedEof, "Unexpected EOF. \\r\\n expected"))
                    )
                } else {
                    Ok((sz, false))
                }
            }
            Ok((sz, LineReadResult::LineFound)) => {
                Ok((sz, true))
            }
            Err(e) => {
                Err(SMTPConnError::from(e))
            }
        }
    }

    /// client_read_response reads response code from SMTP server
    pub fn client_read_response(&mut self, buf: &mut [u8]) -> Result<(usize, bool), SMTPConnError> {
        self.server_read_command(buf)
    }
}
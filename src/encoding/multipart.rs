use std::borrow::Cow;
use std::cmp::max;
use std::collections::VecDeque;
use std::io::{Error, ErrorKind, Read};

use crate::utils::{BoundaryDetector, BoundaryDetectorResult};

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum PartReaderState {
    LookingForBoundary,
    FoundFinalBoundary,
    FoundMiddleBoundary,
}

#[derive(Debug, Copy, Clone, PartialEq)]
enum FinalBoundaryStateMatch {
    None,
    Final,
    Middle,
}

/// PartReader reads single multipart part until it reaches end part
pub struct PartReader<R> {
    reader: R,

    bd: BoundaryDetector<'static>,
    final_bd: BoundaryDetector<'static>,
    middle_bd: BoundaryDetector<'static>,

    previous_char: Option<u8>,

    state: PartReaderState,
    match_state: FinalBoundaryStateMatch,

    recovery_buffer: VecDeque<u8>,
}

impl<R> PartReader<R> {
    pub fn new(reader: R, boundary: &[u8], use_single_nl: bool) -> Self {
        let mut final_boundary = Vec::with_capacity(3);
        let mut middle_boundary = Vec::with_capacity(4);
        let mut common_boundary = Vec::with_capacity(boundary.len() + 4);

        if use_single_nl {
            common_boundary.extend_from_slice(b"\n--");
            common_boundary.extend_from_slice(boundary);

            middle_boundary.extend_from_slice(b"\n");
            final_boundary.extend_from_slice(b"--\n");
        } else {
            common_boundary.extend_from_slice(b"\r\n--");
            common_boundary.extend_from_slice(boundary);

            middle_boundary.extend_from_slice(b"\r\n");
            final_boundary.extend_from_slice(b"--\r\n");
        }
        Self {
            reader,

            state: PartReaderState::LookingForBoundary,
            match_state: FinalBoundaryStateMatch::None,
            recovery_buffer: VecDeque::with_capacity(max(final_boundary.len(), middle_boundary.len())),

            previous_char: None,

            bd: BoundaryDetector::new(Cow::from(common_boundary)),
            final_bd: BoundaryDetector::new(Cow::from(final_boundary)),
            middle_bd: BoundaryDetector::new(Cow::from(middle_boundary)),
        }
    }
}

impl<R> Read for PartReader<R>
    where R: Read
{
    #[allow(clippy::cognitive_complexity)]
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Error> {
        let mut last_buffer_len = buf.len();

        while last_buffer_len > 0 {
            // eprintln!("Loop iteration: last_buffer_len: {}", last_buffer_len);
            match self.state {
                PartReaderState::LookingForBoundary => {}
                _ => break,
            }

            let i = buf.len() - last_buffer_len;
            if let Some(b) = self.recovery_buffer.pop_front() {
                // got byte to put in buf. Decrement counter and put byte.
                buf[i] = b;
                last_buffer_len -= 1;
                continue;
            }
            // TODO(teawithsand) first populate buf with data from self.recovery_buffer

            let b = if let Some(b) = self.previous_char {
                self.previous_char = None;
                b
            } else {
                let mut bb = [0u8; 1];

                // stream may be nonblocking!
                // byte by byte reading is slow though...
                // apply buffered reader on this first
                // TODO(teawithsand) fix case when data is read to buffer and error occurs and state is corrupted now.
                //  either store it in to load buffer or cache error and return it later on next read try.
                let b = self.reader.read(&mut bb)?;
                debug_assert!(b == 0 || b == 1);

                // read len == 0 is eof
                if b == 0 {
                    if self.bd.is_done() {
                        let middle_boundary = self.middle_bd.get_boundary();
                        let final_boundary = self.final_bd.get_boundary();
                        if middle_boundary[middle_boundary.len() - 1] == b'\n' {
                            let sub_len = if middle_boundary[middle_boundary.len() - 2] == b'\r' {
                                2
                            } else {
                                1
                            };
                            // eprintln!("sub_len: {}", sub_len);
                            if self.middle_bd.get_pos() as usize == middle_boundary.len() - sub_len {
                                self.state = PartReaderState::FoundFinalBoundary;
                            }

                            if self.middle_bd.get_pos() as usize == final_boundary.len() - sub_len {
                                self.state = PartReaderState::FoundMiddleBoundary;
                            }
                        }
                    }
                    if let PartReaderState::LookingForBoundary = self.state {
                        return Err(Error::new(ErrorKind::UnexpectedEof, "Reader is done but multipart end was not found"));
                    }
                    break;
                }
                bb[0]
            };
            // eprintln!("Running chr: {:?}", b as char);

            debug_assert!(self.recovery_buffer.is_empty());

            if self.bd.is_done() {
                // eprintln!("BD is done");
                match self.match_state {
                    FinalBoundaryStateMatch::None => {
                        // eprintln!("None matched");
                        let mm = self.middle_bd.pass_byte(b);
                        let fm = self.final_bd.pass_byte(b);
                        match (mm, fm) {
                            (BoundaryDetectorResult::NoMatch, BoundaryDetectorResult::NoMatch) => {
                                // eprintln!("Both not matched");
                                self.recovery_buffer.extend(self.bd.get_boundary());
                                self.previous_char = Some(b);
                                self.bd.reset();
                                continue;
                            }

                            (BoundaryDetectorResult::MatchBegin, BoundaryDetectorResult::NoMatch) => {
                                self.match_state = FinalBoundaryStateMatch::Middle;
                                continue;
                            }
                            (BoundaryDetectorResult::MatchBegin, BoundaryDetectorResult::MatchBroke(_)) => {
                                self.match_state = FinalBoundaryStateMatch::Middle;
                                continue;
                            }

                            (BoundaryDetectorResult::NoMatch, BoundaryDetectorResult::MatchBegin) => {
                                // eprintln!("Final matched first");
                                self.match_state = FinalBoundaryStateMatch::Final;
                                continue;
                            }
                            (BoundaryDetectorResult::MatchBroke(_), BoundaryDetectorResult::MatchBegin) => {
                                // eprintln!("Final second first");
                                self.match_state = FinalBoundaryStateMatch::Final;
                                continue;
                            }

                            (BoundaryDetectorResult::MatchBroke(first_data), BoundaryDetectorResult::MatchBroke(second_data)) => {
                                debug_assert_eq!(first_data, second_data);

                                // eprintln!("Both broken matched");
                                self.recovery_buffer.extend(self.bd.get_boundary());
                                self.recovery_buffer.extend(second_data);
                                self.previous_char = Some(b);
                                self.bd.reset();
                                continue;
                            }
                            _ => panic!("Invalid matches state: middle_boundary: {:?} final_boundary: {:?}", mm, fm),
                        }
                    }
                    FinalBoundaryStateMatch::Middle => {
                        // eprintln!("Middle matched");
                        match self.middle_bd.pass_byte(b) {
                            BoundaryDetectorResult::MatchDone => {
                                // eprintln!("Middle matched and done");
                                self.state = PartReaderState::FoundMiddleBoundary;
                                continue;
                            }
                            BoundaryDetectorResult::MatchBroke(matched_data) => {
                                self.recovery_buffer.extend(self.bd.get_boundary());
                                self.recovery_buffer.extend(matched_data);
                                self.previous_char = Some(b);

                                self.bd.reset();
                                continue;
                            }
                            // yet another char with matches came in
                            BoundaryDetectorResult::MatchBegin => {
                                continue;
                            }
                            BoundaryDetectorResult::NoMatch => panic!("Middle match invalid state: {:?}", BoundaryDetectorResult::NoMatch),
                        };
                    }
                    FinalBoundaryStateMatch::Final => {
                        // eprintln!("Final matched");
                        // eprintln!("final pos: {:?}", self.final_bd.get_pos());
                        match self.final_bd.pass_byte(b) {
                            BoundaryDetectorResult::MatchDone => {
                                // eprintln!("Final matched and done");
                                self.state = PartReaderState::FoundFinalBoundary;
                                continue;
                            }
                            BoundaryDetectorResult::MatchBroke(matched_data) => {
                                self.recovery_buffer.extend(self.bd.get_boundary());
                                self.recovery_buffer.extend(matched_data);
                                self.previous_char = Some(b);

                                self.bd.reset();
                                continue;
                            }
                            // yet another char with matches came in
                            BoundaryDetectorResult::MatchBegin => {
                                continue;
                            }
                            BoundaryDetectorResult::NoMatch => panic!("Final match invalid state: {:?}", BoundaryDetectorResult::NoMatch),
                        };
                    }
                };
            } else {
                debug_assert_eq!(self.final_bd.get_pos(), 0);
                debug_assert_eq!(self.middle_bd.get_pos(), 0);
                match self.bd.pass_byte(b) {
                    BoundaryDetectorResult::MatchDone => {
                        self.match_state = FinalBoundaryStateMatch::None;
                        continue;
                    }
                    BoundaryDetectorResult::MatchBegin => {
                        continue;
                    }
                    BoundaryDetectorResult::MatchBroke(matched_data) => {
                        self.recovery_buffer.extend(matched_data);
                        self.previous_char = Some(b);
                        continue;
                    }
                    BoundaryDetectorResult::NoMatch => {}
                };
            }
            // not a boundary byte. Pass it to buffer and decrement counter.
            last_buffer_len -= 1;
            buf[i] = b;
        }

        Ok(buf.len() - last_buffer_len)
    }
}

#[cfg(test)]
mod test {
    use std::io::Cursor;

    use super::*;

    fn perform_test_deserialize_with_buf_len(i: &str, o: Option<&str>, buf_sz: usize) {
        let text = i.as_bytes();
        // eprintln!("Input: {:?}", i);
        let mut r = Cursor::new(Vec::from(text));
        {
            let mut pr = PartReader::new(&mut r, b"some-boundary", false);
            let mut data = Vec::new();
            if let Some(o) = o {
                let mut buf = Vec::new();
                loop {
                    let mut rd_buf = vec![0u8; buf_sz];
                    let len = pr.read(&mut rd_buf).unwrap();
                    if len == 0 {
                        break;
                    }
                    buf.extend_from_slice(&rd_buf[..len]);
                }
                assert_eq!(&String::from_utf8(buf).unwrap(), o);
            } else {
                pr.read_to_end(&mut data).unwrap_err();
            }
        }
    }

    #[test]
    fn test_can_read_part_form_multipart() {
        for (i, o) in [
            /*(
                concat!(
                "some text",
                "\r\n--some-boundary\r\n"
                ),
                Some("some text")
            ),*/
            (
                concat!(
                "some text",
                "\r\n--some-boundary--\r\n"
                ),
                Some("some text")
            ),
            (
                concat!(
                "some text",
                "\r\n--some-like-boundary\r\n",
                "\r\n--some-boundary\r\n"
                ),
                Some("some text\r\n--some-like-boundary\r\n")
            ),
            /*(
                concat!(
                "some text",
                "\r\n--some-like-boundary\r\n",
                "\r\n--some-boundary--\r\n"
                ),
                Some("some text\r\n--some-like-boundary\r\n")
            ),*/
        ].iter().cloned() {
            for sz in [
                1, 2, 4, 8, 16, 32, 64, 128, 256
            ].iter().cloned() {
                perform_test_deserialize_with_buf_len(i, o, sz);
            }
        }
    }
}
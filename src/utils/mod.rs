use std::borrow::Cow;

pub mod hex;
pub mod quoted;
pub mod cc;
pub mod iter_ext;
// pub mod text_stream;
/*
#[inline]
pub fn read_stream_to_string(r: &mut impl Read) -> Result<String, ()> {
    let mut s = String::new();
    r.read_to_string(&mut s).map_err(|_| ()).map(|_| s)
}

#[inline]
pub fn read_stream_to_buf_string(r: &mut impl Read, buf: &mut String) -> Result<String, ()> {
    r.read_to_string(buf).map_err(|_| ()).map(|_| s)
}
*/

/// BoundaryDetector consumes bytes one by one.
/// It's able to tell whatever or not was boundary reached and how many data
/// has to be read again once boundary read filed but started well.
pub struct BoundaryDetector<'b> {
    boundary: Cow<'b, [u8]>,
    pos: u32,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum BoundaryDetectorResult<'b> {
    NoMatch,
    MatchBegin,
    MatchBroke(&'b [u8]),
    MatchDone,
}

impl<'b> BoundaryDetector<'b> {
    pub fn new(boundary: Cow<'b, [u8]>) -> Self {
        Self {
            boundary,
            pos: 0,
        }
    }

    pub fn is_done(&self) -> bool {
        self.pos as usize == self.boundary.len()
    }

    #[inline]
    pub fn reset(&mut self) {
        self.pos = 0;
    }

    #[inline]
    pub fn get_boundary(&self) -> &[u8] {
        self.boundary.as_ref()
    }

    #[inline]
    pub fn get_pos(&self) -> u32 {
        self.pos
    }

    #[inline]
    pub fn pass_byte(&mut self, b: u8) -> BoundaryDetectorResult<'_> {
        debug_assert!(self.pos as usize <= self.boundary.len());
        if self.pos as usize >= self.boundary.len() {
            return BoundaryDetectorResult::MatchDone;
        }

        if self.boundary[self.pos as usize] == b {
            self.pos += 1;
            if self.pos as usize == self.boundary.len() {
                BoundaryDetectorResult::MatchDone
            } else {
                BoundaryDetectorResult::MatchBegin
            }
        } else if self.pos == 0 {
            BoundaryDetectorResult::NoMatch
        } else {
            let pos = self.pos as usize;
            self.pos = 0;
            BoundaryDetectorResult::MatchBroke(&self.boundary.as_ref()[..pos])
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_partial_match() {
        let mut bd = BoundaryDetector::new(Cow::Borrowed(b"--asdf-boundary"));
        let data = b"some data --asdf-boundar";
        let mut eb = Vec::new();
        let mut mb_cnt = 0usize;
        for b in data.iter().cloned() {
            match bd.pass_byte(b) {
                BoundaryDetectorResult::MatchDone => {
                    panic!("Match done! It should not be");
                }
                BoundaryDetectorResult::MatchBegin => {
                    mb_cnt += 1;
                }
                BoundaryDetectorResult::MatchBroke(v) => {
                    eb.extend_from_slice(v);
                }
                _ => {}
            }
        }
        assert_eq!(mb_cnt, b"--asdf-boundar".len())
    }
}
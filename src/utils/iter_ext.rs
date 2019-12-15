use std::str::Chars;

pub struct CharsOffsetIter<'a> {
    chars: Chars<'a>,
    offset: usize,
}

impl<'a> Iterator for CharsOffsetIter<'a> {
    type Item = (usize, char);

    fn next(&mut self) -> Option<Self::Item> {
        match self.chars.next() {
            None => None,
            Some(v) => {
                self.offset += v.len_utf8();
                Some((self.offset, v))
            }
        }
    }
}

pub trait CharsOffsetEnumerate<'a> {
    fn utf8_offset_enumerate(self) -> CharsOffsetIter<'a>;
}

impl<'a> CharsOffsetEnumerate<'a> for Chars<'a> {
    fn utf8_offset_enumerate(self) -> CharsOffsetIter<'a> {
        CharsOffsetIter {
            chars: self,
            offset: 0,
        }
    }
}
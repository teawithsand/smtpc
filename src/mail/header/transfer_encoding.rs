use std::io::{Error, Read};
use std::str::FromStr;

use crate::encoding::base64::Base64Reader;
use crate::encoding::quoted_printable::QuotedPrintableReader;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[derive(Serialize, Deserialize)]
pub enum ContentTransferEncoding {
    Base64,
    QuotedPrintable,
    EightBitAscii,
    SevenBitAscii,
    Binary,

    /// Any ContentTransferEncoding that was not recognised
    Other,
}

#[derive(From)]
pub enum ContentTransferEncodingDecoder<R> {
    /// NoDecoder is decoder used when either 7bit or 8bit or binary or Other encoding has been applied
    NoDecoder(R),
    Base64(Base64Reader<R>),
    QuotedPrintable(QuotedPrintableReader<R>),
}

impl<R> Read for ContentTransferEncodingDecoder<R>
    where R: Read
{
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Error> {
        match self {
            ContentTransferEncodingDecoder::NoDecoder(r) => r.read(buf),
            ContentTransferEncodingDecoder::Base64(r) => r.read(buf),
            ContentTransferEncodingDecoder::QuotedPrintable(r) => r.read(buf),
        }
    }
}

impl FromStr for ContentTransferEncoding {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let text = s.trim();
        // OPTIMIZE to case insensitive compares instead of reallocation
        let res = match &text.to_ascii_uppercase()[..] {
            "BINARY" => ContentTransferEncoding::Binary,
            "8BIT" => ContentTransferEncoding::EightBitAscii,
            "7BIT" => ContentTransferEncoding::SevenBitAscii,
            "QUOTED-PRINTABLE" => ContentTransferEncoding::QuotedPrintable,
            "BASE64" => ContentTransferEncoding::Base64,
            _ => {
                return Err(());
            }
        };
        Ok(res)
    }
}

impl ContentTransferEncoding {
    pub fn get_decoder<R>(self, r: R) -> ContentTransferEncodingDecoder<R> {
        match self {
            // TODO(teawithsand) introduce spaceless here
            ContentTransferEncoding::Base64 => ContentTransferEncodingDecoder::Base64(Base64Reader::new(r)),
            ContentTransferEncoding::QuotedPrintable => ContentTransferEncodingDecoder::QuotedPrintable(QuotedPrintableReader::new(r)),
            _ => ContentTransferEncodingDecoder::NoDecoder(r)
        }
    }

    /// decode decodes cte from it's content. It can't fail.
    /// If encoding was not recognised `Other` is returned
    pub fn decode(text: &str) -> ContentTransferEncoding {
        match ContentTransferEncoding::from_str(text) {
            Ok(cte) => cte,
            _ => ContentTransferEncoding::Other,
        }
    }
}

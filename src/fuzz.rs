use std::fmt::Debug;
use std::io::{Cursor, Read};
use std::io;

use crate::encoding::base64::Base64Reader;
use crate::encoding::multipart::PartReader;
use crate::encoding::quoted_printable::QuotedPrintableReader;
use crate::mail::address::EmailAddress;
use crate::mail::date::parse_date;
use crate::mail::header::{ParsedMailHeader, RawHeaderBag};
use crate::utils::quoted::parse_rfc_2047;

fn drain_reader(r: &mut impl io::Read) {
    loop {
        let mut buf = [0u8; 32];
        if let Ok(sz) = r.read(&mut buf) {
            if sz == 0 {
                break;
            }
        } else {
            break;
        }
    }
}

pub fn fuzz_multipart(data: &[u8]) {
    let mut reader = Cursor::new(data);
    {
        let mut pr = PartReader::new(&mut reader, b"", false);
        drain_reader(&mut pr);
    }
}

pub fn fuzz_base64_decoder(data: &[u8]) {
    let mut reader = Cursor::new(data);
    {
        let mut d = Base64Reader::new(&mut reader);
        drain_reader(&mut d);
    }
}

pub fn fuzz_quoted_printable_decoder(data: &[u8]) {
    let mut reader = Cursor::new(data);
    {
        let mut d = QuotedPrintableReader::new(&mut reader);
        drain_reader(&mut d);
    }
}

pub fn fuzz_parse_address(data: &[u8]) {
    if let Ok(text) = std::str::from_utf8(data) {
        let _ = EmailAddress::parse_group(text);
    }
}

pub fn fuzz_parse_date(data: &[u8]) {
    if let Ok(text) = std::str::from_utf8(data) {
        let _ = parse_date(text);
    }
}

pub fn fuzz_rfc_2047(data: &[u8]) {
    if let Ok(text) = std::str::from_utf8(data) {
        let _ = parse_rfc_2047(text);
    }
}

pub fn fuzz_mail_raw_mail_header_bag(data: &[u8]) {
    if let Ok(text) = std::str::from_utf8(data) {
        let _ = RawHeaderBag::parse(text);
    }
}

pub fn fuzz_parse_mail_header(data: &[u8]) {
    if let Ok(text) = std::str::from_utf8(data) {
        let mut offset = 0;
        let mut found = false;
        for c in text.chars() {
            if c == ':' {
                found = true;
                break;
            }
            offset += c.len_utf8();
        }
        if !found {
            return;
        }
        if offset == text.len() {
            return;
        }
        let n = &text[..offset];
        let v = &text[offset + 1..];
        // eprintln!("{:?} {:?}", n, v);
        let mh = ParsedMailHeader::try_parse(n, v);
        sink(mh);
    }
}

#[allow(dead_code)]
#[inline(never)]
fn sink<T: Debug>(_v: T) {
    // eprintln!("{:?}", v);
}

/*
pub fn fuzz_serde_json(data: &[u8]) {
    let r: Result<serde_json::Value, _> = serde_json::from_slice(data);
    if let Ok(r) = r {
        let r = serde_json::to_writer(std::io::sink(), &r);
        sink(r);
    }
}

pub fn fuzz_mail_parse(data: &[u8]) {
    let res = mailparse::parse_mail(data);
    sink(res);
}

#[inline(never)]
fn sink<T>(arg: T) {}
*/

// Note on this fuzz test:
// It looks like base64 crate is more liberal about incoming data than for instance python's parser.
// For instance when trying to decode `EWE` base64 seems no problem while python module fails with incorrect padding error.
// Decoder here is also less liberal rather than more. That's why data length has to be multiple of 4.
//
// Note: base64.b64decode was used in python
// And it's even worse. It goes both ways.
// Python's decoder likes `WYQ=YQ==` while base64 signals error with this input.
// Anyway looks like python parser truncates base64 after first padding char found!
// (bug hunters look at you when you use input data with two different parsers).
//
// For instance given is valid b64string in python(unless validate=True flag is set): "WYQ=YQ==asdfasda```s" and it's result is eq to
// `base64.b64decode("WYQ=")`
pub fn fuzz_base64_decoder_gives_same_result(data: &[u8]) {
    if data.len() % 4 != 0 {
        return;
    }
    let mut reader = Cursor::new(data);
    let (res, is_ok) = {
        let mut d = Base64Reader::new(&mut reader);
        let mut res = Vec::new();
        let is_ok = d.read_to_end(&mut res).is_ok();
        (res, is_ok)
    };
    let ok_res = base64::decode_config(data, base64::STANDARD);
    // eprintln!("Base64 res: {} Original res: {}", ok_res.is_ok(), is_ok);
    assert_eq!(ok_res.is_ok(), is_ok);
    if let Ok(ok_res) = ok_res {
        assert_eq!(ok_res, res);
    }
}
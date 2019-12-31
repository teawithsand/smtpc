use std::io::{Cursor, Read};

use smtpc::encoding::quoted_printable::QuotedPrintableReader;
use smtpc::mail::header::{ParsedHeaderBag, RawHeaderBag};
use smtpc::mail::header::count_header_bytes;

const SIMPLE_MAIL: &str = "\
From: Bob <sender@example.com> \r\n\
To: Alice <recipient@example.com> \r\n\
Subject: CTE Test\r\n\
Content-Transfer-Encoding: QUOTED-PRINTABLE\r\n\
\r\n\
=54=68=69=73=20=69=73=20=6a=75=73=74=20=73=61=6d=70=6c=65=20=74=65=78=74=\r\n\
=20=62=75=74=20=71=75=6f=74=65=64=2d=70=72=69=6e=74=61=62=6c=65=20=65=6e=\r\n\
=63=6f=64=65=64\
";

fn main() {
    println!("Parsing plain old mail: ");
    println!("---\n{}\n---", SIMPLE_MAIL);

    // First count how many byes are occupied by headers.
    let header_bytes = count_header_bytes(SIMPLE_MAIL.as_bytes()).unwrap();
    println!("Headers occupy {} bytes", header_bytes);

    let raw_header = RawHeaderBag::parse(&SIMPLE_MAIL[..header_bytes])
        .expect("Header parsing filed");
    // println!("Got raw header bag: {:?}", raw_header);

    let header = ParsedHeaderBag::parse_raw_bag(&raw_header);
    println!("Parsed mail headers: {:#?}", header);

    // we have to trim mail as it may start with OR end with \r\n
    // so now it's prettier
    let mail = &SIMPLE_MAIL[header_bytes+4..];
    println!("Mail contents(RAW): {:#?}", mail);
    {
        let mut r = QuotedPrintableReader::new(Cursor::new(SIMPLE_MAIL[header_bytes+4..].as_bytes()));
        let mut res = String::new();
        r.read_to_string(&mut res).unwrap();
        println!("Mail contents(PARSED: {:#?}", &res);
    }
}

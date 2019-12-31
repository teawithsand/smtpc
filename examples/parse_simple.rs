use smtpc::mail::header::{ParsedHeaderBag, RawHeaderBag};
use smtpc::mail::header::count_header_bytes;

const SIMPLE_MAIL: &str = "\
From: Bob <sender@example.com> \r\n\
To: Alice <recipient@example.com> \r\n\
Subject: Test\r\n\
\r\n\
This one is plain old email with no fancy features like multipart stuff.\
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
    let mail = &SIMPLE_MAIL[header_bytes..].trim();
    println!("Mail contents: {:#?}", mail);
}

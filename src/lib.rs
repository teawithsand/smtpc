//! SMTPC crate provides utilities required to:
//! - parse messages sent over SMTP encoded with quoted-printable, base64
//! - has support for reading multipart messages


#[macro_use]
extern crate derive_more;

#[cfg(feature = "serialize")]
#[macro_use]
extern crate serde_derive;

/*
mod smtp_conn;
mod builder;
mod encoding;

pub mod header_bag;
pub mod mailparse;
pub mod multipart;
*/

pub(crate) mod utils;

#[cfg(fuzzing)]
pub mod fuzz;

pub mod encoding;
pub mod mail;
// pub mod smtp;
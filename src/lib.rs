#[macro_use]
extern crate derive_more;
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
pub mod fuzz;
pub mod encoding;
pub mod mail;
// pub mod smtp;
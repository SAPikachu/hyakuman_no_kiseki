#![feature(vec_resize, convert, ip_addr)]

#[macro_use] mod utils;
pub mod socks5;

#[macro_use] extern crate log;
#[macro_use] extern crate error_type;

extern crate mio;
extern crate mioco;
extern crate byteorder;

use std::io;
use std::io::{Read, ErrorKind, Error};

// try! with logging
macro_rules! tryl {
    ($expr:expr, $level:ident) => (match $expr {
        Ok(val) => val,
        Err(err) => {
            $level!("{}:{}: {} -> {:?}",
                    file!(), line!(), stringify!($expr), err);
            return Err(::std::convert::From::from(err))
        }
    });
    ($expr:expr) => (tryl!($expr, debug));
}

macro_rules! err {
    ($expr:expr) => (Err(::std::convert::From::from($expr)));
}

macro_rules! simple_enum {
    (pub enum $name:ident { $($member:ident,)+ }) => {
        #[derive(Debug)]
        pub enum $name {
            $($member),+
        }
        impl $name {
            fn stringify(&self) -> &'static str {
                match *self {
                    $($name::$member => stringify!($member)),+
                }
            }
        }
        impl ::std::fmt::Display for $name {
            fn fmt(&self, fmt: &mut ::std::fmt::Formatter) -> ::std::result::Result<(), ::std::fmt::Error> {
                fmt.write_str(self.stringify())
            }
        }
    }
}

pub trait ReadUtils {
    fn read_byte(&mut self) -> io::Result<u8>;
    fn read_bytes(&mut self, num_bytes: usize) -> io::Result<Vec<u8>>;
}

impl<T: Read> ReadUtils for T {
    fn read_byte(&mut self) -> io::Result<u8> {
        let mut buf = [0u8];
        match self.read(&mut buf) {
            Ok(1) => Ok(buf[0]),
            Ok(0) => Err(Error::new(ErrorKind::BrokenPipe, "EOF")),
            Ok(_) => unreachable!(),
            Err(x) => Err(x),
        }
    }
    fn read_bytes(&mut self, num_bytes: usize) -> io::Result<Vec<u8>> {
        let mut buf: Vec<u8> = Vec::with_capacity(num_bytes);
        buf.resize(num_bytes, 0);
        let mut remaining_bytes = num_bytes;

        while remaining_bytes > 0 {
            match self.read(&mut buf[(num_bytes - remaining_bytes)..]) {
                Ok(0) => {
                    return Err(Error::new(ErrorKind::BrokenPipe, "EOF"));
                },
                Ok(n) => {
                    assert!(n <= remaining_bytes);
                    remaining_bytes -= n;
                },
                Err(x) => { return Err(x); },
            };
        }

        Ok(buf)
    }
}

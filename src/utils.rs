use std::io;
use std::io::{Read, ErrorKind};

pub trait ReadUtils {
    fn read_byte(&mut self) -> io::Result<u8>;
}

impl<T: Read> ReadUtils for T {
    fn read_byte(&mut self) -> io::Result<u8> {
        let mut buf = [0u8];
        match self.read(&mut buf) {
            Ok(1) => Ok(buf[0]),
            Ok(0) => Err(io::Error::new(ErrorKind::BrokenPipe, "EOF")),
            Ok(_) => unreachable!(),
            Err(x) => Err(x),
        }
    }
}

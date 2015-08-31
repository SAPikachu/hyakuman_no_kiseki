use std::io;
use std::io::{Write, Read};
use std::net::{SocketAddr, ToSocketAddrs, IpAddr};

use mio::tcp::{TcpSocket, TcpStream, Shutdown};
use mioco::MiocoHandle;
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};

use utils::ReadUtils;

simple_enum! {
    pub enum ProtocolError {
        UnsupportedVersion,
        NoSupportedAuth,
        General,
        CommandNotSupported,
        AddressTypeNotSupported,
        HostUnreachable,
    }
}


error_type! {
    #[derive(Debug)]
    pub enum Error {
        Io(io::Error) {
            cause;
        },
        Protocol(ProtocolError) {
            desc (e) e.stringify();
        },
        ByteOrder(::byteorder::Error) {
            cause;
        }
    }
}
impl Into<io::Error> for Error {
    fn into(self) -> io::Error {
        match self {
            Error::Io(e) => e,
            _ => io::Error::new(io::ErrorKind::Other, self),
        }
    }
}
impl Error {
    fn socks_code(&self) -> u8 {
        match self {
            &Error::Protocol(ProtocolError::HostUnreachable) => 4,
            &Error::Protocol(ProtocolError::CommandNotSupported) => 7,
            &Error::Protocol(ProtocolError::AddressTypeNotSupported) => 8,
            &Error::Io(ref e) => match e.kind() {
                io::ErrorKind::ConnectionRefused => 5,
                io::ErrorKind::ConnectionReset => 5,
                io::ErrorKind::PermissionDenied => 2,
                _ => 1,
            },
            _ => 1,
        }
    }
}
type Result<T> = ::std::result::Result<T, Error>;

fn connect_to_remote(address: String, port: u16, mioco: &mut MiocoHandle) -> Result<(TcpStream)> {
    // Note: This may block, consider using a dedicated thread to resolve
    let sockaddrs = try!((address.as_str(), port).to_socket_addrs());
    let mut addr = sockaddrs.filter(
        |x| match x { &SocketAddr::V4(_) => true, _ => false }
    );
    let addr = match addr.next() {
        Some(a) => a,
        None => { return err!(ProtocolError::HostUnreachable); },
    };

    let sock = try!(TcpSocket::v4());
    let (stream, completed) = try!(sock.connect(&addr));
    if completed {
        return Ok(stream);
    }
    let mut stream = mioco.wrap(stream);
    mioco.select_write_from(&[stream.id()]);
    try!(stream.with_raw_mut(|s| s.take_socket_error()));
    Ok(try!(stream.with_raw_mut(|s| s.try_clone())))
}


pub struct Socks5 {
    pub listen_addr: SocketAddr,
}

fn read_socks_version<T: Read>(conn: &mut T) -> Result<()> {
    match try!(conn.read_byte()) {
        5 => Ok(()),
        _ => err!(ProtocolError::UnsupportedVersion),
    }
}
fn reply_error<T: Write, U>(conn: &mut T, error: Error) -> Result<U> {
    try!(conn.write_all(&[
        5u8, // SOCKS version
        error.socks_code(),
        0, // RESERVED
        0, // ATYP
        0, 0, 0, 0, // BND.ADDR
        0, 0, // BND.PORT
    ]));
    Err(error)
}
fn reply_if_error<T: Write, U>(conn: &mut T, result: Result<U>) -> Result<U> {
    match result {
        Ok(x) => Ok(x),
        Err(err) => reply_error(conn, err)
    }
}
fn begin_pipe_data(input: TcpStream, output: TcpStream, mioco: &mut MiocoHandle) {
    mioco.spawn(move |mioco| {
        let input_addr = input.peer_addr().unwrap();
        let output_addr = output.peer_addr().unwrap();
        trace!("Pipe starting, input: {}, output: {}", input_addr, output_addr);
        let mut input = mioco.wrap(input);
        let mut output = mioco.wrap(output);
        let mut buf = [0u8; 65536];
        loop {
            match input.read(&mut buf) {
                Ok(0) => {
                    break;
                },
                Err(e) => {
                    warn!("Error when reading data: {:?}", e);
                    break;
                },
                Ok(n) => {
                    match output.write_all(&buf[..n]) {
                        Ok(_) => {},
                        Err(e) => {
                            warn!("Error when writing data: {:?}", e);
                            break;
                        }
                    }
                }
            };
        }
        output.with_raw_mut(|s| s.shutdown(Shutdown::Write)).unwrap_or(());
        input.with_raw_mut(|s| s.shutdown(Shutdown::Read)).unwrap_or(());
        trace!("Pipe exiting, input: {}, output: {}", input_addr, output_addr);
        Ok(())
    });
}
fn handle_connection(conn: TcpStream, mioco: &mut MiocoHandle) -> Result<()> {
    debug!("New connection, local: {}, remote: {}",
           try!(conn.local_addr()),
           try!(conn.peer_addr()),);
    let mut conn = mioco.wrap(conn);
    // https://www.ietf.org/rfc/rfc1928.txt
    // 1. Method selection
    try!(read_socks_version(&mut conn));
    let nmethods = try!(conn.read_byte()) as usize;
    let methods = try!(conn.read_bytes(nmethods));
    if !methods.contains(&0) {
        try!(conn.write_all(&[5u8, 0xff]));
        return err!(ProtocolError::NoSupportedAuth);
    }
    try!(conn.write_all(&[5u8, 0])); // No authentication

    // 2. Connection request
    try!(read_socks_version(&mut conn));
    if try!(conn.read_byte()) != 1 { // CONNECT
        try!(reply_error(&mut conn, ProtocolError::CommandNotSupported.into()));
    }
    try!(conn.read_byte()); // RESERVED
    let address = match try!(conn.read_byte()) {
        1 => { // IPv4 address
            let b = try!(conn.read_bytes(4));
            format!("{}.{}.{}.{}", b[0], b[1], b[2], b[3])
        },
        3 => {
            let len = try!(conn.read_byte()) as usize;
            let bytes = try!(conn.read_bytes(len));
            try!(String::from_utf8(bytes)
                 .map_err(|_| ProtocolError::AddressTypeNotSupported))
        },
        _ => {
            return err!(ProtocolError::AddressTypeNotSupported);
        },
    };
    let port = try!(conn.read_u16::<BigEndian>());
    debug!("Address: {}, Port: {}", address, port);

    let remote_tx = try!(reply_if_error(&mut conn, connect_to_remote(address, port, mioco)));
    let remote_rx = try!(remote_tx.try_clone());

    let conn_tx = try!(conn.with_raw_mut(|s| s.try_clone()));
    let conn_rx = try!(conn.with_raw_mut(|s| s.try_clone()));

    let local_addr = try!(remote_tx.local_addr());
    let mut reply = vec![5u8, 0, 0, 1];
    match local_addr.ip() {
        IpAddr::V4(addr) => { reply.extend(addr.octets().into_iter()); },
        _ => { try!(reply_error(&mut conn, ProtocolError::General.into())); },
    };
    try!(reply.write_u16::<BigEndian>(local_addr.port()));
    try!(conn.write_all(&reply));

    begin_pipe_data(remote_rx, conn_rx, mioco);
    begin_pipe_data(conn_tx, remote_tx, mioco);

    Ok(())
}
impl Socks5 {
    pub fn run(self, mioco: &mut MiocoHandle) -> io::Result<()> {
        debug!("Socks5 running, listen_addr: {}", self.listen_addr);
        let sock = try!(TcpSocket::v4());
        try!(sock.bind(&self.listen_addr));
        let sock = try!(sock.listen(1024));
        let sock = mioco.wrap(sock);

        loop {
            let conn = try!(sock.accept());
            mioco.spawn(
                move |mioco| handle_connection(conn, mioco)
                             .map_err(|x| x.into())
            );
        }
    }
}

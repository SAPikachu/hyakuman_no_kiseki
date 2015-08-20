use std::io;
use std::net::SocketAddr;

use mio::tcp::{TcpSocket, TcpStream};
use mioco::MiocoHandle;

use utils::ReadUtils;

pub struct Socks5 {
    pub listen_addr: SocketAddr,
}

fn handle_connection(conn: TcpStream, mioco: &mut MiocoHandle) -> io::Result<()> {
    debug!("New connection, local: {}, remote: {}",
           try!(conn.local_addr()),
           try!(conn.peer_addr()),);
    let mut conn = mioco.wrap(conn);
    let socks_ver = try!(conn.read_byte());
    debug!("Socks version: {}", socks_ver);
    Ok(())
}
impl Socks5 {
    pub fn run(&self, mioco: &mut MiocoHandle) -> io::Result<()> {
        debug!("Socks5 running, listen_addr: {}", self.listen_addr);
        let sock = try!(TcpSocket::v4());
        try!(sock.bind(&self.listen_addr));
        let sock = try!(sock.listen(1024));
        let sock = mioco.wrap(sock);

        loop {
            let conn = try!(sock.accept());
            mioco.spawn(move |mioco| handle_connection(conn, mioco));
        }
    }
}

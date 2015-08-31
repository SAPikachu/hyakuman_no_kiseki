extern crate env_logger;
extern crate mioco;

#[macro_use]
extern crate log;

extern crate hyakuman_no_kiseki;

use std::str::FromStr;

use hyakuman_no_kiseki::socks5::Socks5;

fn main() {
    env_logger::init().unwrap();

    debug!("Debugging output enabled");

    mioco::start(move |mioco| {
        let s = Socks5 {
            listen_addr: FromStr::from_str("127.0.0.1:6666").unwrap(),
        };
        mioco.spawn(|mioco| s.run(mioco));
        Ok(())
    });
}

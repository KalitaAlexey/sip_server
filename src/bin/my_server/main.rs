use async_std::task;
use libsip::{Domain, Transport, UriSchema};
use sip_server::Server;
use std::env;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

mod my_client;
mod my_client_factory;
mod my_system;

use my_client_factory::MyClientFactory;

fn main() {
    let mut args = env::args();
    let _ = args.next();
    let ip = args.next();
    let port = args.next();
    let (ip, port) = if let (Some(ip), Some(port)) = (ip, port) {
        (ip.parse::<Ipv4Addr>(), port.parse::<u16>())
    } else {
        eprintln!("<ip> <port>");
        return;
    };
    let (ip, port) = if let (Ok(ip), Ok(port)) = (ip, port) {
        (ip, port)
    } else {
        eprintln!("Invalid <ip> or <port>");
        return;
    };
    env_logger::init();
    let address = SocketAddr::new(IpAddr::V4(ip), port);
    let factory = MyClientFactory::new(
        Transport::Udp,
        UriSchema::Sip,
        Domain::Ipv4(ip, Some(port)),
        true,
    );
    let _ = task::block_on(Server::run(factory, address));
}

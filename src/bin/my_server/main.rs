use async_std::task;
use futures::channel::mpsc;
use libsip::{Domain, Transport, UriSchema};
use sip_server::Server;
use std::env;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

mod my_client;
mod my_client_manager;
mod my_system;

use my_client_manager::MyClientManager;

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
    let addr = SocketAddr::new(IpAddr::V4(ip), port);
    let (sender, receiver) = mpsc::unbounded();
    let manager = MyClientManager::new(
        Transport::Udp,
        UriSchema::Sip,
        Domain::Ipv4(ip, Some(port)),
        sender,
        false,
    );
    let _ = task::block_on(Server::run(manager, receiver, addr));
}

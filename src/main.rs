#![recursion_limit = "1024"]
#![feature(async_closure)]

use async_std::{
    net::{IpAddr, Ipv4Addr},
    task,
};
use futures::channel::mpsc;
use std::net::SocketAddr;

mod client_handler;
mod client_handler_mgr;
mod my_client_handler;
mod my_client_handler_mgr;
mod my_system;
mod server;
mod utils;
mod via_branch_generator;

use libsip::{Domain, Transport, UriSchema};
use my_client_handler_mgr::MyClientHandlerMgr;
use server::Server;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
type Sender<T> = mpsc::UnboundedSender<T>;
type Receiver<T> = mpsc::UnboundedReceiver<T>;

use std::env;

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
    let mgr = MyClientHandlerMgr::new(
        Transport::Udp,
        UriSchema::Sip,
        Domain::Ipv4(ip, Some(port)),
        sender,
    );
    let _ = task::block_on(Server::run(mgr, receiver, addr));
}

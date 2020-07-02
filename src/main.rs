#![recursion_limit = "1024"]
#![feature(async_closure)]

use async_std::{net::Ipv4Addr, task};
use futures::channel::mpsc;

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

fn main() {
    let (sender, receiver) = mpsc::unbounded();
    let mgr = MyClientHandlerMgr::new(
        Transport::Udp,
        UriSchema::Sip,
        Domain::Ipv4(Ipv4Addr::new(192, 168, 0, 20), Some(5060)),
        sender,
    );
    let fut = Server::run(mgr, receiver, "192.168.0.20:5060");
    task::spawn(fut);
    loop {}
}

use crate::{msg_router::MsgRouter, tcp_server::TcpServer, udp_server::UdpServer, ClientFactory};
use async_std::net::SocketAddr;
use futures::{channel::mpsc, join};

pub struct Server;

impl Server {
    pub async fn run<F>(factory: F, addr: SocketAddr)
    where
        F: ClientFactory + 'static,
    {
        let (sender, receiver) = mpsc::unbounded();
        let message_router_fut = MsgRouter::new(receiver).run();

        let factory = unsafe {
            let factory: *const F = &factory;
            let factory: &'static F = &*factory;
            factory
        };

        let udp_server_fut = UdpServer::run(factory, addr, sender.clone());

        let tcp_server = TcpServer::new(factory, addr, sender);
        let tcp_server_fut = tcp_server.run();

        join!(message_router_fut, udp_server_fut, tcp_server_fut);
    }
}

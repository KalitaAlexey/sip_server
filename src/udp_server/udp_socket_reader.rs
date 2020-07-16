use super::{
    udp_client_event_handler::UdpClientEventHandler, udp_socket_writer::UdpSocketWriterMessage,
};
use crate::{
    client_worker::{ClientWorker, ClientWorkerMessage},
    msg_router::MsgRouterMsg,
    sip_parse, ClientFactory, Sender,
};
use async_std::{
    net::{SocketAddr, UdpSocket},
    task::{self, JoinHandle},
};
use futures::{channel::mpsc, SinkExt};
use libsip::SipMessage;
use log::{debug, error};
use std::collections::HashMap;

/// Reads a datagram from UDP socket, parses it into a SIP message and sends it to [`MsgRouter`](../../message_router/struct.MsgRouter.html)
pub(crate) struct UdpSocketReader<'a, F> {
    /// The socket it reads from
    socket: &'a UdpSocket,
    /// The sender it sends SIP messages with
    message_router_sender: Sender<MsgRouterMsg>,
    /// The sender it provides to [`UdpClientEventHandler`](../udp_client_event_handler/struct.UdpClientEventHandler.html) that is created for each new connection
    socket_writer_sender: Sender<UdpSocketWriterMessage>,
    /// The factory it creates [`Client`](../../trait.Client.html) for each new connection
    factory: &'a F,
    /// List of connected clients used to send received messages to
    client_workers: HashMap<SocketAddr, (Sender<ClientWorkerMessage>, JoinHandle<()>)>,
}

impl<'a, F: ClientFactory> UdpSocketReader<'a, F> {
    pub fn new(
        socket: &'a UdpSocket,
        message_router_sender: Sender<MsgRouterMsg>,
        socket_writer_sender: Sender<UdpSocketWriterMessage>,
        factory: &'a F,
    ) -> Self {
        Self {
            socket,
            message_router_sender,
            socket_writer_sender,
            factory,
            client_workers: HashMap::new(),
        }
    }

    pub async fn run(mut self) {
        self.read_msgs().await;
        for (_, (_, handle)) in self.client_workers.into_iter() {
            handle.await;
        }
    }

    async fn read_msgs(&mut self) {
        let mut buffer = [0; 4096];
        loop {
            let (addr, msg) = self.read_msg(&mut buffer).await;
            if !self.client_workers.contains_key(&addr) {
                self.spawn_client_worker(addr).await;
            }
            let (sender, _) = self.client_workers.get_mut(&addr).unwrap();
            Self::send_to_client_worker(msg, sender).await;
        }
    }

    async fn read_msg(&self, buffer: &mut [u8]) -> (SocketAddr, SipMessage) {
        loop {
            match self.socket.recv_from(buffer).await {
                Ok((n, addr)) => {
                    // It's impossible for SIP message to fit in 4 bytes.
                    // However, 3CXPhone sometimes (every 30 seconds) sends a packet whose content is "\r\n\r\n".
                    // Trying to parse such a packet leads to parsing error.
                    // Checking to avoid unnecessary error logs.
                    if n > 4 {
                        debug!("received {} bytes from {}", n, addr);
                        if let Some(msg) = sip_parse::parse(&buffer[..n]) {
                            return (addr, msg);
                        } else {
                            error!("parse failed");
                        }
                    }
                }
                Err(e) => {
                    error!("recv_from failed: {}", e);
                }
            }
        }
    }

    async fn spawn_client_worker(&mut self, addr: SocketAddr) {
        let event_handler = Box::new(UdpClientEventHandler::new(
            addr,
            self.message_router_sender.clone(),
            self.socket_writer_sender.clone(),
        ));
        let client = self.factory.create_client(addr, event_handler);

        let (sender, receiver) = mpsc::unbounded();

        let client_worker = ClientWorker::new(client, receiver);
        let handle = task::spawn(client_worker.run());

        self.register_client_worker(addr, sender.clone()).await;

        self.client_workers.insert(addr, (sender, handle));
    }

    async fn send_to_client_worker(msg: SipMessage, sender: &mut Sender<ClientWorkerMessage>) {
        let msg = ClientWorkerMessage::Received(msg);
        if let Err(e) = sender.send(msg).await {
            error!("send failed: {}", e);
        }
    }

    async fn register_client_worker(
        &mut self,
        addr: SocketAddr,
        sender: Sender<ClientWorkerMessage>,
    ) {
        let msg = MsgRouterMsg::ClientWorker { addr, sender };
        if let Err(e) = self.message_router_sender.send(msg).await {
            error!("send failed: {}", e);
        }
    }
}

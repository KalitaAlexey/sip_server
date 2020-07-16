use super::{
    udp_client_event_handler::UdpClientEventHandler, udp_socket_writer::UdpSocketWriterMessage,
};
use crate::{
    client_worker::{ClientWorker, ClientWorkerMessage},
    message_router::MessageRouterMessage,
    sip_parse, ClientFactory, Sender,
};
use async_std::{
    net::{SocketAddr, UdpSocket},
    task::{self, JoinHandle},
};
use futures::{channel::mpsc, SinkExt};
use libsip::SipMessage;
use log::{debug, error};

/// Reads a datagram from UDP socket, parses it into a SIP message and sends it to [`MessageRouter`](../../message_router/struct.MessageRouter.html)
pub(crate) struct UdpSocketReader<'a, F> {
    /// The socket it reads from
    socket: &'a UdpSocket,
    /// The sender it sends SIP messages with
    message_router_sender: Sender<MessageRouterMessage>,
    /// The sender it provides to [`UdpClientEventHandler`](../udp_client_event_handler/struct.UdpClientEventHandler.html) that is created for each new connection
    socket_writer_sender: Sender<UdpSocketWriterMessage>,
    /// The factory it creates [`Client`](../../trait.Client.html) for each new connection
    factory: &'a F,
    /// List of addresses of all connected clients used to check if a message is received from a new connection
    client_addrs: Vec<SocketAddr>,
    /// Handles to make sure all created client workers are done before [`run`](#method.run) finishes
    client_worker_handles: Vec<JoinHandle<()>>,
}

impl<'a, F: ClientFactory> UdpSocketReader<'a, F> {
    pub fn new(
        socket: &'a UdpSocket,
        message_router_sender: Sender<MessageRouterMessage>,
        socket_writer_sender: Sender<UdpSocketWriterMessage>,
        factory: &'a F,
    ) -> Self {
        Self {
            socket,
            message_router_sender,
            socket_writer_sender,
            factory,
            client_addrs: Vec::new(),
            client_worker_handles: Vec::new(),
        }
    }

    pub async fn run(mut self) {
        self.read_msgs().await;
        for handle in self.client_worker_handles.into_iter() {
            handle.await;
        }
    }

    async fn read_msgs(&mut self) {
        let mut buffer = [0; 4096];
        loop {
            let (addr, msg) = self.read_msg(&mut buffer).await;
            if self.client_addrs.contains(&addr) {
                let msg = MessageRouterMessage::ReceivedMessage { addr, msg };
                self.send_to_message_router(msg).await;
            } else {
                let mut sender = self.create_new_client(addr).await;
                // Received message can be sent directly to client worker since its sender is created here
                let msg = ClientWorkerMessage::Received(msg);
                if let Err(e) = sender.send(msg).await {
                    error!("send failed: {}", e);
                }
            }
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

    async fn create_new_client(&mut self, addr: SocketAddr) -> Sender<ClientWorkerMessage> {
        let sender = self.create_client_worker(addr).await;
        let msg = MessageRouterMessage::ClientWorker {
            addr,
            sender: sender.clone(),
        };
        self.send_to_message_router(msg).await;
        self.client_addrs.push(addr);
        sender
    }

    async fn create_client_worker(&mut self, addr: SocketAddr) -> Sender<ClientWorkerMessage> {
        let event_handler = Box::new(UdpClientEventHandler::new(
            addr,
            self.message_router_sender.clone(),
            self.socket_writer_sender.clone(),
        ));
        let client = self.factory.create_client(addr, event_handler);

        let (sender, receiver) = mpsc::unbounded();

        let client_worker = ClientWorker::new(client, receiver);
        let handle = task::spawn(client_worker.run());
        self.client_worker_handles.push(handle);

        sender
    }

    async fn send_to_message_router(&mut self, msg: MessageRouterMessage) {
        if let Err(e) = self.message_router_sender.send(msg).await {
            error!("send failed: {}", e);
        }
    }
}

use super::{
    udp_client_event_handler::UdpClientEventHandler, udp_socket_writer::UdpSocketWriterMessage,
};
use crate::{
    client_worker::{self, ClientWorkerMessage},
    message_router::MessageRouterMessage,
    sip_parse, ClientFactory, Sender,
};
use async_std::{
    net::{SocketAddr, UdpSocket},
    task::JoinHandle,
};
use futures::{channel::mpsc, SinkExt};
use libsip::SipMessage;
use log::error;

pub(crate) struct UdpSocketReader<'a, F> {
    socket: &'a UdpSocket,
    message_router_sender: Sender<MessageRouterMessage>,
    socket_writer_sender: Sender<UdpSocketWriterMessage>,
    factory: &'a F,
    /// List of addresses of all connected clients to see if a message is received from a new client
    client_addrs: Vec<SocketAddr>,
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
                        if let Some(msg) = sip_parse::parse(&buffer[..n]) {
                            return (addr, msg);
                        } else {
                            error!("parse failed");
                        }
                    }
                }
                Err(e) => {
                    error!("failed: {}", e);
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

        let handle = async_std::task::spawn(client_worker::run(client, receiver));
        self.client_worker_handles.push(handle);

        sender
    }

    async fn send_to_message_router(&mut self, msg: MessageRouterMessage) {
        if let Err(e) = self.message_router_sender.send(msg).await {
            error!("send failed: {}", e);
        }
    }
}

use super::udp_socket_writer::UdpSocketWriterMessage;
use crate::{message_router::MessageRouterMessage, ClientEvent, ClientEventHandler, Sender};
use async_std::net::SocketAddr;
use async_trait::async_trait;
use futures::SinkExt;
use log::error;

/// [`ClientEventHandler`](trait.ClientEventHandler.html) for a client whose transport protocol is UDP
pub(crate) struct UdpClientEventHandler {
    addr: SocketAddr,
    message_router_sender: Sender<MessageRouterMessage>,
    socket_writer_sender: Sender<UdpSocketWriterMessage>,
}

impl UdpClientEventHandler {
    pub fn new(
        addr: SocketAddr,
        message_router_sender: Sender<MessageRouterMessage>,
        socket_writer_sender: Sender<UdpSocketWriterMessage>,
    ) -> Self {
        Self {
            addr,
            message_router_sender,
            socket_writer_sender,
        }
    }
}

#[async_trait]
impl ClientEventHandler for UdpClientEventHandler {
    async fn handle(&mut self, event: ClientEvent) {
        let result = match event {
            ClientEvent::Route { addr, msg } => {
                let msg = MessageRouterMessage::RoutedMessage { addr, msg };
                self.message_router_sender.send(msg).await
            }
            ClientEvent::Send(msg) => {
                let msg = UdpSocketWriterMessage {
                    addr: self.addr,
                    msg,
                };
                self.socket_writer_sender.send(msg).await
            }
        };
        if let Err(e) = result {
            error!("UdpClientEventHandler::handle: send failed: {}", e);
        }
    }
}

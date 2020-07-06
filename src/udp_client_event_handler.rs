use crate::{ClientEvent, ClientEventHandler, Sender};
use async_trait::async_trait;
use futures::SinkExt;
use log::error;

/// [`ClientEventHandler`](trait.ClientEventHandler.html) for a client whose transport protocol is UDP
pub struct UdpClientEventHandler {
    sender: Sender<ClientEvent>,
}

impl UdpClientEventHandler {
    pub fn new(sender: Sender<ClientEvent>) -> Self {
        Self { sender }
    }
}

#[async_trait]
impl ClientEventHandler for UdpClientEventHandler {
    async fn handle(&mut self, event: ClientEvent) {
        if let Err(e) = self.sender.send(event).await {
            error!("UdpClientEventHandler::handle: send failed: {}", e);
        }
    }
}

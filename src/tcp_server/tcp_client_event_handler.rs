use crate::{msg_router::MsgRouterMsg, ClientEvent, ClientEventHandler, Sender};
use async_std::{net::TcpStream, prelude::*, sync::Arc};
use async_trait::async_trait;
use futures::SinkExt;
use log::error;

pub(crate) struct TcpClientEventHandler {
    stream: Arc<TcpStream>,
    sender: Sender<MsgRouterMsg>,
}

impl<'a> TcpClientEventHandler {
    pub fn new(stream: Arc<TcpStream>, sender: Sender<MsgRouterMsg>) -> Self {
        Self { stream, sender }
    }
}

#[async_trait]
impl ClientEventHandler for TcpClientEventHandler {
    async fn handle(&mut self, event: ClientEvent) {
        match event {
            ClientEvent::Route { addr, msg } => {
                let msg = MsgRouterMsg::RoutedMessage { addr, msg };
                if let Err(e) = self.sender.send(msg).await {
                    error!("send failed: {}", e);
                }
            }
            ClientEvent::Send(message) => {
                let message = message.to_string();
                let bytes = message.as_bytes();
                let mut stream: &TcpStream = &self.stream;
                if let Err(e) = stream.write_all(bytes).await {
                    error!("write_all failed: {}", e);
                }
            }
        }
    }
}

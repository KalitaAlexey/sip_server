use async_std::net::SocketAddr;
use async_trait::async_trait;
use libsip::SipMessage;

use crate::Result;

pub enum ClientHandlerMsg {
    SendToClient(SocketAddr, SipMessage),
}

#[async_trait]
pub trait ClientHandler {
    async fn on_msg(&mut self, msg: SipMessage) -> Result<()>;
}

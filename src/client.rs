use crate::Result;
use async_std::net::SocketAddr;
use async_trait::async_trait;
use libsip::SipMessage;

#[async_trait]
pub trait Client {
    /// May return another address and some message to be handled by another client
    async fn on_message(&mut self, message: SipMessage) -> Result<Option<(SocketAddr, SipMessage)>>;

    async fn on_routed_message(&mut self, message: SipMessage) -> Result<()>;
}

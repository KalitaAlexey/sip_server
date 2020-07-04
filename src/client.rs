use crate::Result;
use async_trait::async_trait;
use libsip::SipMessage;

#[async_trait]
pub trait Client {
    async fn on_message(&mut self, message: SipMessage) -> Result<()>;
}

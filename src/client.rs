use async_trait::async_trait;
use libsip::SipMessage;

#[async_trait]
pub trait Client: Send + Sync {
    async fn on_msg(&mut self, msg: SipMessage);

    async fn on_routed_msg(&mut self, msg: SipMessage);
}

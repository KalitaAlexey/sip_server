use async_trait::async_trait;
use libsip::SipMessage;

use crate::{BrokerMsg, Result, Sender};

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct ClientHandlerId(pub(crate) u32);

pub enum ClientHandlerMsg {
    SendToClient(ClientHandlerId, SipMessage),
    SendToBroker(BrokerMsg),
}

#[async_trait]
pub trait ClientHandler {
    fn new(id: ClientHandlerId, sender: Sender<ClientHandlerMsg>) -> Self;
    fn id(&self) -> ClientHandlerId;
    async fn on_msg(&mut self, msg: SipMessage) -> Result<()>;
}

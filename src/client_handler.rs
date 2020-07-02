use async_std::sync::Mutex;
use async_trait::async_trait;
use libsip::{Domain, SipMessage, Transport, UriSchema};
use std::sync::Arc;

use crate::{utils::Utils, BrokerMsg, Result, Sender};

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct ClientHandlerId(pub(crate) u32);

pub enum ClientHandlerMsg {
    SendToClient(ClientHandlerId, SipMessage),
    SendToBroker(BrokerMsg),
}

#[async_trait]
pub trait ClientHandler {
    type System;

    fn new(
        id: ClientHandlerId,
        transport: Transport,
        schema: UriSchema,
        domain: Domain,
        utils: Arc<Utils>,
        sender: Sender<ClientHandlerMsg>,
        system: Arc<Mutex<Self::System>>,
    ) -> Self;

    fn id(&self) -> ClientHandlerId;

    async fn on_msg(&mut self, msg: SipMessage) -> Result<()>;
}

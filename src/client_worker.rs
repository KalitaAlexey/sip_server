use crate::{Client, Receiver};
use futures::StreamExt;
use libsip::SipMessage;

pub(crate) enum ClientWorkerMessage {
    Received(SipMessage),
    Routed(SipMessage),
}

pub(crate) struct ClientWorker {
    client: Box<dyn Client>,
    receiver: Receiver<ClientWorkerMessage>,
}

impl ClientWorker {
    pub fn new(client: Box<dyn Client>, receiver: Receiver<ClientWorkerMessage>) -> Self {
        Self { client, receiver }
    }

    pub async fn run<'a>(mut self) {
        while let Some(msg) = self.receiver.next().await {
            match msg {
                ClientWorkerMessage::Received(msg) => self.client.on_msg(msg).await,
                ClientWorkerMessage::Routed(msg) => self.client.on_routed_msg(msg).await,
            }
        }
    }
}

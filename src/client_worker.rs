use crate::{Client, Receiver};
use futures::StreamExt;
use libsip::SipMessage;

pub(crate) enum ClientWorkerMessage {
    Received(SipMessage),
    Routed(SipMessage),
}

pub(crate) async fn run<'a>(
    mut client: Box<dyn Client + 'a>,
    mut receiver: Receiver<ClientWorkerMessage>,
) {
    while let Some(msg) = receiver.next().await {
        match msg {
            ClientWorkerMessage::Received(msg) => client.on_msg(msg).await,
            ClientWorkerMessage::Routed(msg) => client.on_routed_msg(msg).await,
        }
    }
}

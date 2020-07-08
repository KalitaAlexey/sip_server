use crate::Receiver;
use async_std::net::{SocketAddr, UdpSocket};
use futures::StreamExt;
use libsip::SipMessage;
use log::error;

pub(crate) struct UdpSocketWriterMessage {
    pub addr: SocketAddr,
    pub msg: SipMessage,
}

pub(crate) async fn run(socket: &UdpSocket, mut receiver: Receiver<UdpSocketWriterMessage>) {
    while let Some(UdpSocketWriterMessage { addr, msg }) = receiver.next().await {
        let msg = msg.to_string();
        let bytes = msg.as_bytes();
        if let Err(e) = socket.send_to(bytes, addr).await {
            error!("send_to failed: {}", e);
        }
    }
}

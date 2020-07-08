use super::msg_read;
use crate::{client_worker::ClientWorkerMessage, Sender};
use async_std::{net::TcpStream, sync::Arc};
use futures::SinkExt;
use log::error;

pub(crate) async fn run(stream: Arc<TcpStream>, mut sender: Sender<ClientWorkerMessage>) {
    let mut buffer = [0; 4096];
    let mut stream: &TcpStream = &stream;
    loop {
        let msg = msg_read::read_msg(&mut stream, &mut buffer).await;
        let msg = ClientWorkerMessage::Received(msg);
        if let Err(e) = sender.send(msg).await {
            error!("send failed: {}", e);
        }
    }
}

use super::{tcp_client_event_handler::TcpClientEventHandler, tcp_stream_reader};
use crate::{
    client_worker::{ClientWorker, ClientWorkerMessage},
    message_router::MessageRouterMessage,
    ClientFactory, Sender,
};
use async_std::{
    net::{SocketAddr, TcpStream},
    sync::Arc,
    task::{self, JoinHandle},
};
use futures::{channel::mpsc, SinkExt};
use libsip::SipMessage;
use log::error;

pub(crate) struct TcpStreamWorker<F: 'static> {
    addr: SocketAddr,
    stream: Arc<TcpStream>,
    factory: &'static F,
    sender: Sender<MessageRouterMessage>,
}

impl<F: ClientFactory + 'static> TcpStreamWorker<F> {
    pub fn new(
        addr: SocketAddr,
        stream: TcpStream,
        factory: &'static F,
        sender: Sender<MessageRouterMessage>,
    ) -> Self {
        Self {
            addr,
            stream: Arc::new(stream),
            factory,
            sender,
        }
    }

    pub async fn run(mut self, msg: SipMessage) {
        let (client_worker_handle, mut client_worker_sender) = self.spawn_client_worker();

        let cwm = ClientWorkerMessage::Received(msg);
        if let Err(e) = client_worker_sender.send(cwm).await {
            error!("failed to send cwm: {}", e);
        }

        let mrm = MessageRouterMessage::ClientWorker {
            addr: self.addr,
            sender: client_worker_sender.clone(),
        };
        if let Err(e) = self.sender.send(mrm).await {
            error!("failed to send mrm: {}", e);
        }

        tcp_stream_reader::run(self.stream, client_worker_sender).await;

        client_worker_handle.await;
    }

    fn spawn_client_worker(&self) -> (JoinHandle<()>, Sender<ClientWorkerMessage>) {
        let handler = Box::new(TcpClientEventHandler::new(
            self.stream.clone(),
            self.sender.clone(),
        ));
        let client = self.factory.create_client(self.addr, handler);

        let (sender, receiver) = mpsc::unbounded();

        let client_worker = ClientWorker::new(client, receiver);
        let handle = task::spawn(client_worker.run());

        (handle, sender)
    }
}

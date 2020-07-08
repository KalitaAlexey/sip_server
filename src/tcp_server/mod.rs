use crate::{message_router::MessageRouterMessage, ClientFactory, Sender};
use async_std::{
    net::{SocketAddr, TcpListener, TcpStream},
    task::{self, JoinHandle},
};
use futures::StreamExt;
use log::{error, info};
mod msg_read;
mod tcp_client_event_handler;
mod tcp_stream_reader;
mod tcp_stream_waiting_worker;
mod tcp_stream_worker;

pub(crate) struct TcpServer<F: 'static> {
    factory: &'static F,
    addr: SocketAddr,
    sender: Sender<MessageRouterMessage>,
}

impl<F: ClientFactory + 'static> TcpServer<F> {
    pub fn new(
        factory: &'static F,
        addr: SocketAddr,
        sender: Sender<MessageRouterMessage>,
    ) -> Self {
        Self {
            factory,
            addr,
            sender,
        }
    }

    pub async fn run(self) {
        let mut worker_handles = Vec::new();
        self.listen_incoming(&mut worker_handles).await;
        for handle in worker_handles.into_iter() {
            handle.await;
        }
    }

    async fn listen_incoming(&self, worker_handles: &mut Vec<JoinHandle<()>>) {
        let listener = TcpListener::bind(self.addr)
            .await
            .expect("failed to bind tcp listener");
        let mut incoming = listener.incoming();
        while let Some(stream) = incoming.next().await {
            match stream {
                Ok(stream) => self.on_stream(stream, worker_handles),
                Err(e) => error!("tcp stream error: {}", e),
            }
        }
    }

    fn on_stream(&self, stream: TcpStream, worker_handles: &mut Vec<JoinHandle<()>>) {
        match stream.peer_addr() {
            Ok(addr) => {
                info!("new tcp connection: {}", addr);
                let fut =
                    tcp_stream_waiting_worker::run(addr, stream, self.factory, self.sender.clone());
                worker_handles.push(task::spawn(fut));
            }
            Err(e) => {
                error!("peer_addr failed: {}", e);
            }
        }
    }
}

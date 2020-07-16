use crate::{msg_router::MsgRouterMsg, ClientFactory, Sender};
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
    sender: Sender<MsgRouterMsg>,
    worker_handles: Vec<JoinHandle<()>>,
}

impl<F: ClientFactory + 'static> TcpServer<F> {
    pub fn new(factory: &'static F, addr: SocketAddr, sender: Sender<MsgRouterMsg>) -> Self {
        Self {
            factory,
            addr,
            sender,
            worker_handles: Vec::new(),
        }
    }

    pub async fn run(mut self) {
        self.listen_incoming().await;
        for handle in self.worker_handles.into_iter() {
            handle.await;
        }
    }

    async fn listen_incoming(&mut self) {
        let listener = TcpListener::bind(self.addr)
            .await
            .expect("failed to bind tcp listener");
        let mut incoming = listener.incoming();
        while let Some(stream) = incoming.next().await {
            match stream {
                Ok(stream) => self.on_stream(stream),
                Err(e) => error!("tcp stream error: {}", e),
            }
        }
    }

    fn on_stream(&mut self, stream: TcpStream) {
        match stream.peer_addr() {
            Ok(addr) => {
                info!("new tcp connection: {}", addr);
                let fut =
                    tcp_stream_waiting_worker::run(addr, stream, self.factory, self.sender.clone());
                self.worker_handles.push(task::spawn(fut));
            }
            Err(e) => {
                error!("peer_addr failed: {}", e);
            }
        }
    }
}

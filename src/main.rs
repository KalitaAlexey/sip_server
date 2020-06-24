#![recursion_limit = "1024"]

use async_std::{
    prelude::*,
    sync::{Arc, Mutex},
    task,
};
use futures::{channel::mpsc, select, FutureExt};
use nom::error::VerboseError;
use std::net::{SocketAddr, UdpSocket};

mod client_handler;
mod my_client_handler;
mod utils;

use client_handler::{ClientHandler, ClientHandlerId, ClientHandlerMsg};
use my_client_handler::MyClientHandler;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
type Sender<T> = mpsc::UnboundedSender<T>;
type Receiver<T> = mpsc::UnboundedReceiver<T>;

pub enum BrokerMsg {
    NewClient,
}

fn main() -> Result<()> {
    let fut = Server::<MyClientHandler>::run("127.0.0.1:8080");
    task::block_on(fut)
}

pub struct Server<H: ClientHandler> {
    socket: Mutex<UdpSocket>,
    handlers: Mutex<Vec<(SocketAddr, H)>>,
    next_handler_id: Mutex<u32>,
    sender: Sender<ClientHandlerMsg>,
    receiver: Mutex<Receiver<ClientHandlerMsg>>,
}

impl<H: ClientHandler + Send + 'static> Server<H> {
    pub async fn run(addr: &str) -> Result<()> {
        let socket = UdpSocket::bind(addr)?;
        let server = Arc::new(Server::<H>::new(socket));
        let fut = Self::work_with_connection(server);
        spawn_and_log_error(fut);
        loop {}
    }

    fn new(socket: UdpSocket) -> Self {
        let (sender, receiver) = mpsc::unbounded();
        Self {
            socket: Mutex::new(socket),
            handlers: Mutex::new(Vec::new()),
            next_handler_id: Mutex::new(0),
            sender,
            receiver: Mutex::new(receiver),
        }
    }

    async fn work_with_connection(self: Arc<Self>) -> Result<()> {
        let mut buffer = [0; 4096];
        loop {
            let mut receiver = self.receiver.lock().await;
            let receiver = &mut *receiver;
            select! {
                msg = receiver.next().fuse() => match msg {
                    Some(msg) => self.on_msg(msg).await?,
                    _ => {}
                },
                res = async {
                    let socket = self.socket.lock().await;
                    socket.recv_from(&mut buffer)
                }.fuse() => match res {
                    Ok((n, addr)) if n > 0 => self.on_data(&addr, &buffer[..n]).await?,
                    _ => {}
                },
            }
        }
    }

    async fn on_msg(self: &Arc<Self>, msg: ClientHandlerMsg) -> Result<()> {
        match msg {
            ClientHandlerMsg::SendToClient(id, msg) => {
                if let Some(addr) = self.addr_for_id(id).await {
                    let socket = self.socket.lock().await;
                    socket.send_to(msg.to_string().as_bytes(), addr)?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    async fn addr_for_id(self: &Arc<Self>, id: ClientHandlerId) -> Option<SocketAddr> {
        let handlers = self.handlers.lock().await;
        if let Some((addr, _)) = handlers.iter().find(|(_, h)| h.id() == id) {
            Some(addr.clone())
        } else {
            None
        }
    }

    async fn next_id(self: &Arc<Self>) -> u32 {
        let mut id = self.next_handler_id.lock().await;
        let taken_id = *id;
        *id = taken_id + 1;
        taken_id
    }

    async fn on_data(self: &Arc<Self>, addr: &SocketAddr, buffer: &[u8]) -> Result<()> {
        self.create_handler_for_addr(addr).await;
        let (_, msg) = match libsip::parse_message::<VerboseError<&[u8]>>(&buffer) {
            Ok((rest, msg)) => (rest, msg),
            Err(nom::Err::Error(e)) => {
                let s = String::from_utf8_lossy(&e.errors[0].0);
                eprintln!("Errors: {:?}, Kind: {:?}", s, e.errors[0].1);
                return Ok(());
            }
            _ => return Ok(()),
        };
        let mut handlers = self.handlers.lock().await;
        let handler = handlers.iter_mut().find(|(a, _)| a == addr).map(|(_, h)| h);
        if let Some(handler) = handler {
            handler.on_msg(msg).await?;
        }
        Ok(())
    }

    async fn create_handler_for_addr(self: &Arc<Self>, addr: &SocketAddr) {
        let mut handlers = self.handlers.lock().await;
        if handlers.iter().find(|(a, _)| a == addr).is_none() {
            let id = self.next_id().await;
            let handler = H::new(ClientHandlerId(id), self.sender.clone());
            handlers.push((addr.clone(), handler));
        }
    }
}

fn spawn_and_log_error<F>(fut: F) -> task::JoinHandle<()>
where
    F: Future<Output = Result<()>> + Send + 'static,
{
    task::spawn(async move {
        if let Err(e) = fut.await {
            eprintln!("{}", e)
        }
    })
}

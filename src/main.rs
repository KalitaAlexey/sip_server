#![recursion_limit = "1024"]
#![feature(async_closure)]

use async_std::{
    prelude::*,
    sync::{Arc, Mutex},
    task,
};
use futures::{channel::mpsc, select, FutureExt};
use libsip::{Domain, Transport, UriSchema};
use nom::error::VerboseError;
use std::{
    net::{Ipv4Addr, SocketAddr, UdpSocket},
    time::Duration,
};

mod client_handler;
mod my_client_handler;
mod my_system;
mod system;
mod utils;
mod via_branch_generator;

use client_handler::{ClientHandler, ClientHandlerId, ClientHandlerMsg};
use my_client_handler::MyClientHandler;
use my_system::MySystem;
use system::System;
use utils::Utils;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
type Sender<T> = mpsc::UnboundedSender<T>;
type Receiver<T> = mpsc::UnboundedReceiver<T>;

pub enum BrokerMsg {
    NewClient,
}

fn main() -> Result<()> {
    let fut = Server::<MySystem, MyClientHandler>::run("127.0.0.1:5060");
    task::spawn(fut);
    loop {}
}

struct ClientHandlerInfo<H: ClientHandler> {
    id: ClientHandlerId,
    addr: SocketAddr,
    handler: H,
}

pub struct Server<S: System, H: ClientHandler> {
    socket: Mutex<UdpSocket>,
    handlers: Mutex<Vec<ClientHandlerInfo<H>>>,
    next_handler_id: Mutex<u32>,
    utils: Arc<Utils>,
    sender: Sender<ClientHandlerMsg>,
    receiver: Mutex<Receiver<ClientHandlerMsg>>,
    system: Arc<Mutex<S>>,
}

impl<S: System + Send + 'static, H: ClientHandler<System=S> + Send + 'static> Server<S, H> {
    pub async fn run(addr: &str) -> Result<()> {
        let socket = UdpSocket::bind(addr)?;
        socket
            .set_read_timeout(Some(Duration::from_millis(100)))
            .expect("failed to set read timeout");
        let server = Arc::new(Server::<S, H>::new(socket));
        let fut = Self::work_with_connection(server);
        spawn_and_log_error(fut).await;
        Ok(())
    }

    fn new(socket: UdpSocket) -> Self {
        let (sender, receiver) = mpsc::unbounded();
        Self {
            socket: Mutex::new(socket),
            handlers: Mutex::new(Vec::new()),
            next_handler_id: Mutex::new(0),
            utils: Arc::new(Utils::new()),
            sender,
            receiver: Mutex::new(receiver),
            system: Arc::new(Mutex::new(S::new())),
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
                    Err(e) if e.kind() == std::io::ErrorKind::TimedOut => task::yield_now().await,
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
        for ClientHandlerInfo { id: h_id, addr, .. } in handlers.iter() {
            if h_id == &id {
                return Some(addr.clone());
            }
        }
        None
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
        if let Some(handler) = Self::handler_mut_for_addr(&mut handlers, addr) {
            handler.on_msg(msg).await?;
        }
        Ok(())
    }

    async fn create_handler_for_addr(self: &Arc<Self>, addr: &SocketAddr) {
        let mut handlers = self.handlers.lock().await;
        if Self::handler_mut_for_addr(&mut handlers, addr).is_none() {
            let id = self.next_id().await;
            let id = ClientHandlerId(id);
            let handler = H::new(
                id,
                Transport::Udp,
                UriSchema::Sip,
                Domain::Ipv4(Ipv4Addr::new(127, 0, 0, 1), Some(5060)),
                self.utils.clone(),
                self.sender.clone(),
                self.system.clone(),
            );
            handlers.push(ClientHandlerInfo {
                id: id,
                addr: addr.clone(),
                handler,
            });
        }
    }

    fn handler_mut_for_addr<'a>(
        handlers: &'a mut Vec<ClientHandlerInfo<H>>,
        addr: &SocketAddr,
    ) -> Option<&'a mut H> {
        for ClientHandlerInfo {
            addr: h_addr,
            handler,
            ..
        } in handlers
        {
            if h_addr == addr {
                return Some(handler);
            }
        }
        None
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

use async_std::{
    prelude::*,
    sync::{Arc, Mutex},
    task,
};
use futures::{select, FutureExt};

use nom::error::VerboseError;
use std::{
    net::{SocketAddr, UdpSocket},
    time::Duration,
};

use crate::client_handler::{ClientHandler, ClientHandlerMsg};
use crate::client_handler_mgr::ClientHandlerMgr;
use crate::Receiver;
use crate::Result;

pub struct Server<Mgr> {
    mgr: Mutex<Mgr>,
    socket: Mutex<UdpSocket>,
    receiver: Mutex<Receiver<ClientHandlerMsg>>,
}

impl<Mgr: ClientHandlerMgr + Send + 'static> Server<Mgr> {
    pub async fn run(mgr: Mgr, receiver: Receiver<ClientHandlerMsg>, addr: &str) -> Result<()> {
        let socket = UdpSocket::bind(addr)?;
        socket
            .set_read_timeout(Some(Duration::from_millis(100)))
            .expect("failed to set read timeout");
        let server = Self {
            mgr: Mutex::new(mgr),
            socket: Mutex::new(socket),
            receiver: Mutex::new(receiver),
        };
        let server = Arc::new(server);
        let fut = Self::work_with_connection(server);
        spawn_and_log_error(fut).await;
        Ok(())
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
            ClientHandlerMsg::SendToClient(addr, msg) => {
                self.socket
                    .lock()
                    .await
                    .send_to(msg.to_string().as_bytes(), addr)?;
            }
        }
        Ok(())
    }

    async fn on_data(self: &Arc<Self>, addr: &SocketAddr, buffer: &[u8]) -> Result<()> {
        let (_, msg) = match libsip::parse_message::<VerboseError<&[u8]>>(&buffer) {
            Ok((rest, msg)) => (rest, msg),
            Err(nom::Err::Error(e)) => {
                let s = String::from_utf8_lossy(&e.errors[0].0);
                eprintln!("Errors: {:?}, Kind: {:?}", s, e.errors[0].1);
                return Ok(());
            }
            _ => return Ok(()),
        };
        let mut mgr = self.mgr.lock().await;
        let handler = mgr.get_handler(addr.clone());
        handler.on_msg(msg).await?;
        Ok(())
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

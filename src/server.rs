use async_std::{
    net::{SocketAddr, UdpSocket},
    sync::{Arc, Mutex},
    task,
};
use log::error;
use nom::error::VerboseError;

use crate::client_handler::{ClientHandler, ClientHandlerMsg};
use crate::client_handler_mgr::ClientHandlerMgr;
use crate::Receiver;
use futures::StreamExt;

pub struct Server<Mgr> {
    mgr: Mutex<Mgr>,
    socket: UdpSocket,
}

impl<Mgr: ClientHandlerMgr + Send + 'static> Server<Mgr> {
    pub async fn run(mgr: Mgr, receiver: Receiver<ClientHandlerMsg>, addr: SocketAddr) {
        let socket = std::net::UdpSocket::bind(addr).expect("failed to bind udp socket");
        let socket = UdpSocket::from(socket);
        let server = Self {
            mgr: Mutex::new(mgr),
            socket,
        };
        let server = Arc::new(server);
        let handle1 = task::spawn(server.clone().read_data());
        let handle2 = task::spawn(server.read_msgs(receiver));
        handle1.await;
        handle2.await;
    }

    async fn read_data(self: Arc<Self>) {
        let mut buffer = [0; 4096];
        loop {
            match self.socket.recv_from(&mut buffer).await {
                Ok((n, addr)) => {
                    // It's impossible for SIP message to fit in 4 bytes.
                    // However, 3CXPhone sometimes (every 30 seconds) sends a packet whose content is "\r\n\r\n".
                    // Trying to parse such a packet leads to parsing error.
                    // Checking to avoid unnecessary error logs.
                    if n > 4 {
                        self.on_data(addr, &buffer[..n]).await;
                    }
                }
                Err(e) => {
                    error!("recv_from failed: {}", e);
                }
            }
        }
    }

    async fn read_msgs(self: Arc<Self>, mut receiver: Receiver<ClientHandlerMsg>) {
        while let Some(msg) = receiver.next().await {
            self.on_msg(msg).await;
        }
    }

    async fn on_msg(&self, msg: ClientHandlerMsg) {
        match msg {
            ClientHandlerMsg::SendToClient(addr, msg) => {
                if let Err(e) = self.socket.send_to(msg.to_string().as_bytes(), addr).await {
                    error!("send_to failed: {}", e);
                }
            }
        }
    }

    async fn on_data(&self, addr: SocketAddr, buffer: &[u8]) {
        match libsip::parse_message::<VerboseError<&[u8]>>(&buffer) {
            Ok((_, msg)) => {
                let mut mgr = self.mgr.lock().await;
                let handler = mgr.get_handler(addr);
                if let Err(e) = handler.on_msg(msg).await {
                    error!("handler.on_msg failed: {}", e);
                }
            }
            Err(e) => {
                error!("libsip::parse_message failed: {}", e);
            }
        };
    }
}

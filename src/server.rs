use async_std::net::{SocketAddr, UdpSocket};
use log::error;
use nom::error::VerboseError;

use crate::client_handler::{ClientHandler, ClientHandlerMsg};
use crate::client_handler_mgr::ClientHandlerMgr;
use crate::Receiver;
use futures::{join, StreamExt};

pub struct Server;

impl Server {
    pub async fn run<Mgr: ClientHandlerMgr>(
        mgr: Mgr,
        receiver: Receiver<ClientHandlerMsg>,
        addr: SocketAddr,
    ) {
        let socket = std::net::UdpSocket::bind(addr).expect("failed to bind udp socket");
        let socket = UdpSocket::from(socket);
        let socket_reader = SocketReader {
            socket: &socket,
            mgr,
        };
        let msg_reader = MsgReader {
            socket: &socket,
            receiver,
        };
        join!(socket_reader.run(), msg_reader.run());
    }
}

pub struct SocketReader<'a, Mgr> {
    socket: &'a UdpSocket,
    mgr: Mgr,
}

impl<'a, Mgr: ClientHandlerMgr> SocketReader<'a, Mgr> {
    pub async fn run(mut self) {
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

    async fn on_data(&mut self, addr: SocketAddr, buffer: &[u8]) {
        match libsip::parse_message::<VerboseError<&[u8]>>(&buffer) {
            Ok((_, msg)) => {
                let handler = self.mgr.get_handler(addr);
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

pub struct MsgReader<'a> {
    socket: &'a UdpSocket,
    receiver: Receiver<ClientHandlerMsg>,
}

impl<'a> MsgReader<'a> {
    pub async fn run(mut self) {
        while let Some(msg) = self.receiver.next().await {
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
}

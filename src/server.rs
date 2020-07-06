use crate::{Client, ClientEvent, ClientManager, Receiver};
use async_std::net::{SocketAddr, UdpSocket};
use futures::{join, StreamExt};
use log::error;
use nom::error::VerboseError;

pub struct Server;

impl Server {
    pub async fn run<M: ClientManager>(
        manager: M,
        receiver: Receiver<ClientEvent>,
        addr: SocketAddr,
    ) {
        let socket = UdpSocket::bind(addr)
            .await
            .expect("failed to bind udp socket");
        let socket_reader = SocketWorker {
            socket: &socket,
            manager,
        };
        let msg_reader = ClientEventWorker {
            socket: &socket,
            receiver,
        };
        join!(socket_reader.run(), msg_reader.run());
    }
}

pub struct SocketWorker<'a, M> {
    socket: &'a UdpSocket,
    manager: M,
}

impl<'a, M: ClientManager> SocketWorker<'a, M> {
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
            Ok((_, msg)) => match self.manager.get_client(addr).on_message(msg).await {
                Ok(result) => {
                    if let Some((addr, msg)) = result {
                        if let Err(e) = self.manager.get_client(addr).on_routed_message(msg).await {
                            error!("client.on_routed_message failed: {}", e);
                        }
                    }
                }
                Err(e) => {
                    error!("client.on_message failed: {}", e);
                }
            },
            Err(nom::Err::Error(VerboseError { errors })) => {
                for e in errors {
                    error!(
                        "Error {:?} happened with input {}",
                        e.1,
                        std::str::from_utf8(e.0).expect("from_utf8 failed")
                    );
                }
            }
            Err(e) => error!("libsip::parse_message failed: {}", e),
        };
    }
}

pub struct ClientEventWorker<'a> {
    socket: &'a UdpSocket,
    receiver: Receiver<ClientEvent>,
}

impl<'a> ClientEventWorker<'a> {
    pub async fn run(mut self) {
        while let Some(event) = self.receiver.next().await {
            self.on_event(event).await;
        }
    }

    async fn on_event(&self, event: ClientEvent) {
        match event {
            ClientEvent::Send(addr, message) => {
                if let Err(e) = self
                    .socket
                    .send_to(message.to_string().as_bytes(), addr)
                    .await
                {
                    error!("send_to failed: {}", e);
                }
            }
        }
    }
}

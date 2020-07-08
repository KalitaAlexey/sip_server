mod udp_client_event_handler;
mod udp_socket_reader;
mod udp_socket_writer;

use self::udp_socket_reader::UdpSocketReader;
use crate::{message_router::MessageRouterMessage, ClientFactory, Sender};
use async_std::net::{SocketAddr, UdpSocket};
use futures::{channel::mpsc, join};

pub(crate) struct UdpServer;

impl UdpServer {
    pub async fn run<F: ClientFactory>(
        factory: &F,
        address: SocketAddr,
        message_router_sender: Sender<MessageRouterMessage>,
    ) {
        let socket = UdpSocket::bind(address)
            .await
            .expect("failed to bind udp socket");

        let (socket_writer_sender, socket_writer_receiver) = mpsc::unbounded();

        let socket_reader_fut = UdpSocketReader::new(
            &socket,
            message_router_sender.clone(),
            socket_writer_sender,
            factory,
        )
        .run();

        let socket_writer_fut = udp_socket_writer::run(&socket, socket_writer_receiver);

        join!(socket_reader_fut, socket_writer_fut);
    }
}

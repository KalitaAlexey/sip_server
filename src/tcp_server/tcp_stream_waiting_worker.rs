use super::{msg_read, tcp_stream_worker::TcpStreamWorker};
use crate::{message_router::MessageRouterMessage, ClientFactory, Sender};
use async_std::net::{SocketAddr, TcpStream};

pub(crate) async fn run<F: ClientFactory + 'static>(
    addr: SocketAddr,
    stream: TcpStream,
    factory: &'static F,
    sender: Sender<MessageRouterMessage>,
) {
    let mut buffer = [0; 4096];
    let msg = msg_read::read_msg(&mut &stream, &mut buffer).await;

    let worker = TcpStreamWorker::new(addr, stream, factory, sender);
    worker.run(msg).await;
}

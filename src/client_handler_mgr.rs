use async_std::net::SocketAddr;

use crate::client_handler::ClientHandler;

pub trait ClientHandlerMgr {
    type Handler: ClientHandler + Send + 'static;

    /// Creates (if needed) a handler for a given address and returns it
    fn get_handler(&mut self, addr: SocketAddr) -> &mut Self::Handler;
}

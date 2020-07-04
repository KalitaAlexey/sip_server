use crate::client::Client;
use async_std::net::SocketAddr;

pub trait ClientManager {
    type Client: Client;

    /// Creates (if needed) a client for a given address and returns it
    fn get_client(&mut self, addr: SocketAddr) -> &mut Self::Client;
}

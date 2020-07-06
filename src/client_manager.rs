use crate::{Client, ClientEventHandler};
use async_std::net::SocketAddr;

/// The server will ask this manager for a client that will handle messages coming from the address the client works with
pub trait ClientManager {
    type Client: Client;

    /// Creates a new client. The server ensures that this function won't be called some client exists for `address`
    /// # Parameters
    /// * `address` - An address that a client will work with
    /// * `event_handler` - An event handler that providess a mechanism for a client to communicate with the server, e.g., send messages to the client's connection
    fn create_client(
        &mut self,
        address: SocketAddr,
        event_handler: Box<dyn ClientEventHandler>,
    ) -> &mut Self::Client;

    /// Returns a client that works with `address`
    fn get_client(&mut self, address: &SocketAddr) -> Option<&mut Self::Client>;
}

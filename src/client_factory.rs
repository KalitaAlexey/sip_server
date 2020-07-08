use crate::{Client, ClientEventHandler};
use async_std::net::SocketAddr;

/// The server will ask the factory to create a client when a new connection established.
/// The created client will handle message coming from the connection
pub trait ClientFactory: Send + Sync {
    /// Creates a new client. The server ensures that this function won't be called if some client exists for `address`
    /// # Parameters
    /// * `addr` - The address of the connection that the created client will receive messages from
    /// * `event_handler` - The event handler that provides the only mechanism for the created client to communicate with the server
    fn create_client<'a>(
        &self,
        addr: SocketAddr,
        event_handler: Box<dyn ClientEventHandler + 'a>,
    ) -> Box<dyn Client + 'a>;
}

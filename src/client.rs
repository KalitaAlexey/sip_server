use async_std::net::SocketAddr;
use async_trait::async_trait;
use libsip::SipMessage;

#[async_trait]
pub trait Client: Send + Sync {
    async fn on_msg(&mut self, msg: SipMessage);

    async fn on_routed_msg(&mut self, msg: SipMessage);
}

/// The server will ask the factory to create a client when a new connection established.
/// The created client will handle message coming from the connection
pub trait ClientFactory: Send + Sync {
    /// Creates a new client. The server ensures that this function won't be called if some client exists for `address`
    /// # Parameters
    /// * `addr` - The address of the connection that the created client will receive messages from
    /// * `event_handler` - The event handler that provides the only mechanism for the created client to communicate with the server
    fn create_client(
        &self,
        addr: SocketAddr,
        event_handler: Box<dyn ClientEventHandler>,
    ) -> Box<dyn Client>;
}

#[derive(Debug)]
pub enum ClientEvent {
    /// Sends the message to the client's socket
    Send(SipMessage),
    /// Routes the message to be handled by the client whose connection's address matches
    Route { addr: SocketAddr, msg: SipMessage },
}

#[async_trait]
/// Handles [`ClientEvent`](enum.ClientEvent.html) provided by [`Client`](trait.Client.html)
pub trait ClientEventHandler: Send + Sync {
    async fn handle(&mut self, event: ClientEvent);
}

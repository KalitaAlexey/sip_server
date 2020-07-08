use async_std::net::SocketAddr;
use libsip::SipMessage;

#[derive(Debug)]
pub enum ClientEvent {
    /// Sends the message to the client's socket
    Send(SipMessage),
    /// Routes the message to be handled by the client whose connection's address matches
    Route { addr: SocketAddr, msg: SipMessage },
}

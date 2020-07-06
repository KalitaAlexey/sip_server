use async_std::net::SocketAddr;
use libsip::SipMessage;

#[derive(Debug)]
pub enum ClientEvent {
    /// Sends the message to the client socket connection whose address matches
    Send(SocketAddr, SipMessage),
}

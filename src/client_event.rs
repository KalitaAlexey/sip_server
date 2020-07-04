use async_std::net::SocketAddr;
use libsip::SipMessage;

#[derive(Debug)]
pub enum ClientEvent {
    Send(SocketAddr, SipMessage),
}

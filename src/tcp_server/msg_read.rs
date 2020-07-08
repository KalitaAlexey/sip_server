use crate::sip_parse;
use async_std::{io::ReadExt, net::TcpStream};
use libsip::SipMessage;
use log::error;

pub async fn read_msg(stream: &mut &TcpStream, buffer: &mut [u8]) -> SipMessage {
    loop {
        let packet_len = match stream.read(buffer).await {
            Ok(n) => {
                // It's impossible for SIP message to fit in 4 bytes.
                // However, 3CXPhone sometimes (every 30 seconds) sends a packet whose content is "\r\n\r\n".
                // Trying to parse such a packet leads to parsing error.
                // Checking to avoid unnecessary error logs.
                if n > 4 {
                    n
                } else {
                    continue;
                }
            }
            Err(e) => {
                error!("read failed: {}", e);
                continue;
            }
        };
        if let Some(msg) = sip_parse::parse(&buffer[..packet_len]) {
            return msg;
        } else {
            error!("parse failed");
        }
    }
}

use libsip::SipMessage;
use log::error;
use nom::{error::VerboseError, Err};

pub(crate) fn parse(buffer: &[u8]) -> Option<SipMessage> {
    match libsip::parse_message::<VerboseError<&[u8]>>(buffer) {
        Ok((_, msg)) => Some(msg),
        Err(e) => {
            on_parse_error(e);
            None
        }
    }
}

fn on_parse_error(err: Err<VerboseError<&[u8]>>) {
    if let nom::Err::Error(VerboseError { errors }) = err {
        for e in errors {
            error!(
                "error {:?} happened with input {}",
                e.1,
                std::str::from_utf8(e.0).expect("from_utf8 failed")
            );
        }
    } else {
        error!("failed to parse sip message: {}", err);
    }
}

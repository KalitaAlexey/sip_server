use async_std::{sync::Mutex, task};
use futures::SinkExt;
use libsip::{Header, SipMessage};
use std::time::Duration;

use crate::{client_handler::ClientHandlerMsg, via_branch_generator::ViaBranchGenerator, Sender};

pub struct Utils {
    via_branch_generator: Mutex<ViaBranchGenerator>,
}

impl Utils {
    pub fn new() -> Self {
        Self {
            via_branch_generator: Mutex::new(ViaBranchGenerator::new()),
        }
    }

    pub async fn via_branch(&self) -> String {
        self.via_branch_generator.lock().await.branch()
    }
}

pub fn set_to_tag(msg: &mut SipMessage, tag: &str) {
    if let SipMessage::Response {
        ref mut headers, ..
    } = msg
    {
        let h = headers
            .0
            .iter_mut()
            .find_map(|h| if let Header::To(h) = h { Some(h) } else { None });
        if let Some(h) = h {
            h.set_param("tag", Some(tag));
        }
    }
}

pub async fn delay_and_send_msg(
    mut sender: Sender<ClientHandlerMsg>,
    delay_secs: u64,
    msg: ClientHandlerMsg,
) {
    task::sleep(Duration::from_secs(delay_secs)).await;
    if let Err(e) = sender.send(msg).await {
        eprintln!("delay_and_send_msg: {:?}", e);
    }
}

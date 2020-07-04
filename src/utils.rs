use crate::via_branch_generator::ViaBranchGenerator;
use crate::{ClientEvent, Sender};
use async_std::{sync::Mutex, task};
use futures::SinkExt;
use std::time::Duration;

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

pub async fn delay_and_send_msg(
    mut sender: Sender<ClientEvent>,
    delay_secs: u64,
    msg: ClientEvent,
) {
    task::sleep(Duration::from_secs(delay_secs)).await;
    if let Err(e) = sender.send(msg).await {
        eprintln!("delay_and_send_msg: {:?}", e);
    }
}

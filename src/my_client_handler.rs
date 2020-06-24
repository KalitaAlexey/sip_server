use async_trait::async_trait;
use futures::sink::SinkExt;
use libsip::{ResponseGenerator, SipMessage};

use crate::{
    client_handler::{ClientHandler, ClientHandlerId, ClientHandlerMsg},
    utils, Result, Sender,
};

pub struct MyClientHandler {
    id: ClientHandlerId,
    sender: Sender<ClientHandlerMsg>,
}

#[async_trait]
impl ClientHandler for MyClientHandler {
    fn new(id: ClientHandlerId, sender: Sender<ClientHandlerMsg>) -> Self {
        Self { id, sender }
    }

    fn id(&self) -> ClientHandlerId {
        self.id
    }

    async fn on_msg(&mut self, msg: SipMessage) -> Result<()> {
        if let SipMessage::Request { headers, .. } = msg {
            let mut res = ResponseGenerator::new()
                .code(200)
                .headers(headers.into_iter().collect())
                .build()
                .expect("failed to generate response");
            utils::set_to_branch(&mut res, "123456");
            self.sender
                .send(ClientHandlerMsg::SendToClient(self.id, res))
                .await?;
        }
        Ok(())
    }
}

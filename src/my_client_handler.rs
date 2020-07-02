use async_std::{sync::Mutex, task};
use async_trait::async_trait;
use futures::sink::SinkExt;
use libsip::{
    Domain, Header, Method, NamedHeader, RequestGenerator, ResponseGenerator, SipMessage,
    Transport, Uri, UriAuth, UriParam, UriSchema, ViaHeader,
};
use std::sync::Arc;

use crate::{
    client_handler::{ClientHandler, ClientHandlerId, ClientHandlerMsg},
    utils::{self, Utils},
    my_system::MySystem,
    Result, Sender,
};

pub struct MyClientHandler {
    id: ClientHandlerId,
    transport: Transport,
    schema: UriSchema,
    domain: Domain,
    utils: Arc<Utils>,
    sender: Sender<ClientHandlerMsg>,
    system: Arc<Mutex<MySystem>>,
}

#[async_trait]
impl ClientHandler for MyClientHandler {
    type System = MySystem;

    fn new(
        id: ClientHandlerId,
        transport: Transport,
        schema: UriSchema,
        domain: Domain,
        utils: Arc<Utils>,
        sender: Sender<ClientHandlerMsg>,
        system: Arc<Mutex<Self::System>>
    ) -> Self {
        Self {
            id,
            transport,
            schema,
            domain,
            utils,
            sender,
            system
        }
    }

    fn id(&self) -> ClientHandlerId {
        self.id
    }

    async fn on_msg(&mut self, msg: SipMessage) -> Result<()> {
        let method = match &msg {
            SipMessage::Request { method, .. } => method,
            SipMessage::Response { .. } => {
                return Ok(());
            }
        };
        match method {
            Method::Subscribe => self.on_subscribe(msg).await,
            Method::Invite => self.on_invite(msg).await,
            Method::Ack => Ok(()),
            _ => self.on_req(msg).await,
        }
    }
}

impl MyClientHandler {
    async fn on_req(&mut self, msg: SipMessage) -> Result<()> {
        self.send_res(&msg, 200).await
    }

    async fn on_invite(&mut self, msg: SipMessage) -> Result<()> {
        self.send_res(&msg, 180).await
    }

    async fn on_subscribe(&mut self, msg: SipMessage) -> Result<()> {
        self.send_res(&msg, 200).await?;
        if let SipMessage::Request { uri, headers, .. } = msg {
            let uri_username = if let Some(auth) = uri.auth {
                auth.username
            } else {
                eprintln!("no `username` in uri");
                return Ok(());
            };
            let from_hdr = if let Some(Header::From(from_hdr)) = headers.from() {
                from_hdr
            } else {
                eprintln!("no `From`");
                return Ok(());
            };
            let to_hdr = if let Some(Header::To(to_hdr)) = headers.to() {
                to_hdr.param("tag", "123456")
            } else {
                eprintln!("no `To`");
                return Ok(());
            };
            let call_id = if let Some(Header::CallId(call_id)) = headers.call_id() {
                call_id
            } else {
                eprintln!("no `Call-ID`");
                return Ok(());
            };
            let event = if let Some(Header::Event(event)) = headers.event() {
                event
            } else {
                eprintln!("no `Event`");
                return Ok(());
            };
            let uri = Uri::new(self.schema, self.domain.clone());
            let uri = uri.auth(UriAuth::new(uri_username));
            let req = RequestGenerator::new()
                .method(Method::Notify)
                .uri(uri)
                .header(self.via_hdr().await)
                .header(Header::From(to_hdr))
                .header(Header::To(from_hdr))
                .header(Header::Event(event))
                .header(Header::MaxForwards(70))
                .header(Header::CallId(call_id))
                .header(Header::CSeq(50, Method::Notify))
                .header(self.contact_hdr())
                .header(Header::Other(
                    String::from("Subscription-State"),
                    String::from("active;expires=86400"),
                ))
                .build()
                .expect("failed to generate notify");
            let msg = ClientHandlerMsg::SendToClient(self.id, req);
            let sender = self.sender.clone();
            // Maybe it should be saved so it can be waited for before shutdown
            task::spawn(utils::delay_and_send_msg(sender, 2, msg));
        }
        Ok(())
    }

    async fn send_res(&mut self, req: &SipMessage, code: u32) -> Result<()> {
        if let SipMessage::Request { headers, .. } = req {
            let res_hdrs = headers
                .0
                .iter()
                .filter_map(|h| {
                    if let Header::Contact(_) = h {
                        None
                    } else {
                        Some(h.clone())
                    }
                })
                .collect();
            let mut res = ResponseGenerator::new()
                .code(code)
                .headers(res_hdrs)
                .header(self.contact_hdr())
                .build()
                .expect("failed to generate response");
            utils::set_to_tag(&mut res, "123456");
            self.send_to_client(res.clone()).await?;
        }
        Ok(())
    }

    async fn send_to_client(&mut self, msg: SipMessage) -> Result<()> {
        let msg = ClientHandlerMsg::SendToClient(self.id, msg);
        self.sender.send(msg).await?;
        Ok(())
    }

    fn contact_hdr(&self) -> Header {
        Header::Contact(NamedHeader::new(Uri::new(self.schema, self.domain.clone())))
    }

    async fn via_hdr(&self) -> Header {
        let via_branch = self.utils.via_branch().await;
        let via_uri = Uri::new_schemaless(self.domain.clone());
        let via_uri = via_uri.parameter(UriParam::Branch(via_branch));
        Header::Via(ViaHeader::new(via_uri, self.transport))
    }
}

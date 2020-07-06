use crate::my_system::MySystem;
use async_std::{net::SocketAddr, sync::Mutex};
use async_trait::async_trait;
use futures::sink::SinkExt;
use libsip::{
    Domain, Header, Method, NamedHeader, RegisterRequestExt, RequestGenerator, ResponseGenerator,
    SipMessage, SipMessageExt, SubscriptionState, Transport, Uri, UriAuth, UriParam, UriSchema,
    ViaHeader,
};
use log::{debug, warn};
use sip_server::{Client, ClientEvent, IncompleteDialogInfo, DialogInfo, Result, Sender, Utils};
use std::{collections::HashMap, sync::Arc};

pub struct MyClient {
    addr: SocketAddr,
    transport: Transport,
    schema: UriSchema,
    domain: Domain,
    utils: Arc<Utils>,
    sender: Sender<ClientEvent>,
    system: Arc<Mutex<MySystem>>,
}

#[async_trait]
impl Client for MyClient {
    async fn on_message(&mut self, msg: SipMessage) -> Result<Option<(SocketAddr, SipMessage)>> {
        let (request, method) = match &msg {
            SipMessage::Request { method, .. } => (true, *method),
            SipMessage::Response { headers, .. } => {
                if let Some(Header::CSeq(_, method)) = headers.cseq() {
                    (false, method)
                } else {
                    warn!("no `CSeq` in response");
                    return Ok(None);
                }
            }
        };
        if request {
            match method {
                Method::Register => {
                    self.on_register(msg).await?;
                    Ok(None)
                }
                Method::Subscribe => {
                    self.on_subscribe(msg).await?;
                    Ok(None)
                }
                Method::Invite | Method::Bye | Method::Ack | Method::Cancel => {
                    self.route_request(msg).await
                }
                _ => {
                    self.on_req(msg).await?;
                    Ok(None)
                }
            }
        } else {
            match method {
                Method::Invite | Method::Bye | Method::Cancel => self.route_response(msg).await,
                _ => Ok(None),
            }
        }
    }

    async fn on_routed_message(&mut self, message: SipMessage) -> Result<()> {
        if message.is_request() {
            self.on_routed_request(message).await
        } else {
            self.on_routed_response(message).await
        }
    }
}

impl MyClient {
    pub fn new(
        addr: SocketAddr,
        transport: Transport,
        schema: UriSchema,
        domain: Domain,
        utils: Arc<Utils>,
        sender: Sender<ClientEvent>,
        system: Arc<Mutex<MySystem>>,
    ) -> Self {
        Self {
            addr,
            transport,
            schema,
            domain,
            utils,
            sender,
            system,
        }
    }

    async fn on_req(&mut self, msg: SipMessage) -> Result<()> {
        self.send_res(&msg, 200).await
    }

    async fn on_register(&mut self, msg: SipMessage) -> Result<()> {
        let username = if let Some(username) = msg.to_header_username() {
            username
        } else {
            warn!("on_register: no `username` in `To`");
            return self.send_res(&msg, 400).await;
        };
        let expires = if let Some(expires) = msg.expires() {
            expires
        } else {
            warn!("on_register: no `Expires`");
            return self.send_res(&msg, 400).await;
        };
        self.prepare_and_send_res(&msg, 200, |generator| {
            generator.header(Header::Expires(expires))
        })
        .await?;
        let mut system = self.system.lock().await;
        if expires > 0 {
            system
                .registrations
                .register_user(username.clone(), self.addr);
        } else {
            system.registrations.unregister_user(&username);
        }
        Ok(())
    }

    async fn route_request(&mut self, mut msg: SipMessage) -> Result<Option<(SocketAddr, SipMessage)>> {
        let callee = if let Some(callee) = msg.to_header_username() {
            callee
        } else {
            warn!("route_request: no `username` in `To`");
            self.send_res(&msg, 400).await?;
            return Ok(None);
        };
        if msg.via_header_branch().is_none() {
            warn!("route_request: no `branch` in `Via`");
            self.send_res(&msg, 400).await?;
            return Ok(None);
        };
        let callee_addr = {
            let system = self.system.lock().await;
            system.registrations.user_addr(&callee)
        };
        let callee_addr = if let Some(callee_addr) = callee_addr {
            debug!("route_request: callee \"{}\" is registered", callee);
            callee_addr
        } else {
            debug!("route_request: callee \"{}\" isn't registered", callee);
            self.send_res(&msg, 404).await?;
            return Ok(None);
        };
        if msg.to_header_tag().is_none() {
            let (from_tag, call_id) = {
                let from_tag = if let Some(from_tag) = msg.from_header_tag() {
                    from_tag
                } else {
                    warn!("route_request: no `tag` in `From`");
                    self.send_res(&msg, 400).await?;
                    return Ok(None);
                };
                let call_id = if let Some(call_id) = msg.call_id() {
                    call_id
                } else {
                    warn!("route_request: no `Call-ID`");
                    self.send_res(&msg, 400).await?;
                    return Ok(None);
                };
                (from_tag.clone(), call_id.clone())
            };
            let new_to_tag = "toabcd1234".to_string();
            let new_from_tag = "fromabcd1234".to_string();
            msg.set_from_header_tag(new_from_tag.clone());
            let new_call_id = "cidabcd1234".to_string();
            *msg.call_id_mut().unwrap() = new_call_id.clone();

            let incomplete_dialog = IncompleteDialogInfo::new(new_call_id, new_from_tag);
            let dialog = DialogInfo::new(call_id, new_to_tag, from_tag);
            let mut system = self.system.lock().await;
            system.dialogs.add(dialog, incomplete_dialog);
        }
        Ok(Some((callee_addr, msg)))
    }

    async fn on_routed_request(&mut self, mut msg: SipMessage) -> Result<()> {
        let via_branch = if let Some(via_branch) = msg.via_header_branch() {
            via_branch.clone()
        } else {
            warn!("on_routed_request: no `branch` in `Via`");
            return Ok(());
        };
        if let Some(h) = msg.via_header_mut() {
            *h = self.via_hdr_with_branch(via_branch.clone());
        }
        if let Some(h) = msg.contact_header_mut() {
            *h = self.contact_hdr();
        }
        self.sender.send(ClientEvent::Send(self.addr, msg)).await?;
        Ok(())
    }

    async fn on_routed_response(&mut self, msg: SipMessage) -> Result<()> {
        self.sender.send(ClientEvent::Send(self.addr, msg)).await?;
        Ok(())
    }

    async fn route_response(
        &mut self,
        msg: SipMessage,
    ) -> Result<Option<(SocketAddr, SipMessage)>> {
        let caller = if let Some(caller) = msg.from_header_username() {
            caller
        } else {
            warn!("route_response: no `username` in `From`");
            return Ok(None);
        };
        let caller_addr = {
            let system = self.system.lock().await;
            system.registrations.user_addr(&caller)
        };
        if let Some(caller_addr) = caller_addr {
            debug!("route_response: caller \"{}\" is registered", caller);
            Ok(Some((caller_addr, msg)))
        } else {
            debug!("route_response: caller \"{}\" isn't registered", caller);
            Ok(None)
        }
    }

    async fn on_subscribe(&mut self, msg: SipMessage) -> Result<()> {
        self.send_res(&msg, 200).await?;
        if let SipMessage::Request { uri, headers, .. } = msg {
            let uri_username = if let Some(auth) = uri.auth {
                auth.username
            } else {
                warn!("on_subscribe: no `username` in uri");
                return Ok(());
            };
            let from_hdr = if let Some(Header::From(from_hdr)) = headers.from() {
                from_hdr
            } else {
                warn!("on_subscribe: no `From`");
                return Ok(());
            };
            let to_hdr = if let Some(Header::To(to_hdr)) = headers.to() {
                to_hdr.param("tag", Some("123456"))
            } else {
                warn!("on_subscribe: no `To`");
                return Ok(());
            };
            let call_id = if let Some(Header::CallId(call_id)) = headers.call_id() {
                call_id
            } else {
                warn!("on_subscribe: no `Call-ID`");
                return Ok(());
            };
            let event = if let Some(Header::Event(event)) = headers.event() {
                event
            } else {
                warn!("on_subscribe: no `Event`");
                return Ok(());
            };
            let uri = Uri::new(self.schema, self.domain.clone());
            let uri = uri.auth(UriAuth::new(uri_username));
            let req = RequestGenerator::new()
                .method(Method::Notify)
                .uri(uri)
                .header(Header::Via(self.via_hdr().await))
                .header(Header::From(to_hdr))
                .header(Header::To(from_hdr))
                .header(Header::Event(event))
                .header(Header::MaxForwards(70))
                .header(Header::CallId(call_id))
                .header(Header::CSeq(50, Method::Notify))
                .header(Header::Contact(self.contact_hdr()))
                .header(Header::SubscriptionState(SubscriptionState::Active {
                    expires: Some(86400),
                    parameters: HashMap::new(),
                }))
                .header(Header::ContentLength(0))
                .build()
                .expect("failed to generate notify");
            self.send_to_client(req).await?;
        }
        Ok(())
    }

    async fn prepare_and_send_res<F>(&mut self, req: &SipMessage, code: u32, f: F) -> Result<()>
    where
        F: FnOnce(ResponseGenerator) -> ResponseGenerator,
    {
        let generator = self.create_response_generator(req, code);
        let generator = f(generator);
        let res = generator.build().expect("failed to generate response");
        self.send_to_client(res).await?;
        Ok(())
    }

    async fn send_res(&mut self, req: &SipMessage, code: u32) -> Result<()> {
        let res = self
            .create_response_generator(req, code)
            .build()
            .expect("failed to generate response");
        self.send_to_client(res).await
    }

    fn create_response_generator(&self, req: &SipMessage, code: u32) -> ResponseGenerator {
        if let SipMessage::Request { headers, .. } = req {
            let headers = headers
                .0
                .iter()
                .filter_map(|h| match h {
                    Header::Contact(_) | Header::ContentLength(_) => None,
                    Header::To(h) => Some(Header::To(h.clone().param("tag", Some("123456")))),
                    // Via must be kept: 17.1.3 Matching Responses to Client Transactions
                    _ => Some(h.clone()),
                })
                .collect();
            ResponseGenerator::new()
                .code(code)
                .headers(headers)
                // 8.1.1.8 Contact "URI at which the UA would like to receive requests"
                .header(Header::Contact(self.contact_hdr()))
                // 20.14 Content-Length "If no body is present in a message, then the Content-Length header field value MUST be set to zero"
                .header(Header::ContentLength(0))
        } else {
            panic!("not request");
        }
    }

    async fn send_to_client(&mut self, msg: SipMessage) -> Result<()> {
        let msg = ClientEvent::Send(self.addr, msg);
        self.sender.send(msg).await?;
        Ok(())
    }

    fn contact_hdr(&self) -> NamedHeader {
        NamedHeader::new(Uri::new(self.schema, self.domain.clone()))
    }

    async fn via_hdr(&self) -> ViaHeader {
        let via_branch = self.utils.via_branch().await;
        self.via_hdr_with_branch(via_branch)
    }

    fn via_hdr_with_branch(&self, branch: String) -> ViaHeader {
        let via_uri = Uri::new_schemaless(self.domain.clone());
        let via_uri = via_uri.parameter(UriParam::Branch(branch));
        ViaHeader::new(via_uri, self.transport)
    }
}

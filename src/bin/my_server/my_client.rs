use crate::my_system::MySystem;
use async_std::{net::SocketAddr, sync::Mutex};
use async_trait::async_trait;
use futures::sink::SinkExt;
use libsip::{
    Domain, Header, Method, NamedHeader, RegisterRequestExt, RequestGenerator, ResponseGenerator,
    SipMessage, SipMessageExt, SubscriptionState, Transport, Uri, UriAuth, UriParam, UriSchema,
    ViaHeader,
};
use log::{debug, error};
use sip_server::{Client, ClientEvent, DialogInfo, IncompleteDialogInfo, Result, Sender, Utils};
use std::{collections::HashMap, sync::Arc};

pub struct MyClient {
    addr: SocketAddr,
    transport: Transport,
    schema: UriSchema,
    domain: Domain,
    utils: Arc<Utils>,
    sender: Sender<ClientEvent>,
    system: Arc<Mutex<MySystem>>,
    back_to_back: bool,
}

#[async_trait]
impl Client for MyClient {
    async fn on_message(
        &mut self,
        message: SipMessage,
    ) -> Result<Option<(SocketAddr, SipMessage)>> {
        if let Some(method) = message.method() {
            if message.is_request() {
                match method {
                    Method::Register => {
                        self.on_register(message).await?;
                        Ok(None)
                    }
                    Method::Subscribe => {
                        self.on_subscribe(message).await?;
                        Ok(None)
                    }
                    Method::Invite | Method::Bye | Method::Ack | Method::Cancel => {
                        self.route_request(message).await
                    }
                    _ => {
                        self.on_req(message).await?;
                        Ok(None)
                    }
                }
            } else {
                match method {
                    Method::Invite | Method::Bye | Method::Cancel => {
                        self.route_response(message).await
                    }
                    _ => Ok(None),
                }
            }
        } else {
            error!("on_message: no method");
            Ok(None)
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
        back_to_back: bool,
    ) -> Self {
        Self {
            addr,
            transport,
            schema,
            domain,
            utils,
            sender,
            system,
            back_to_back,
        }
    }

    async fn on_req(&mut self, message: SipMessage) -> Result<()> {
        self.send_res(&message, 200).await
    }

    async fn on_register(&mut self, message: SipMessage) -> Result<()> {
        let username = if let Some(username) = message.to_header_username() {
            username
        } else {
            error!("on_register: no `username` in `To`");
            return self.send_res(&message, 400).await;
        };
        let expires = if let Some(expires) = message.expires() {
            expires
        } else {
            error!("on_register: no `Expires`");
            return self.send_res(&message, 400).await;
        };
        self.prepare_and_send_res(&message, 200, |generator| {
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

    async fn route_request(
        &mut self,
        mut message: SipMessage,
    ) -> Result<Option<(SocketAddr, SipMessage)>> {
        if message.via_header_branch().is_none() {
            error!("route_request: no `branch` in `Via`");
            self.send_res(&message, 400).await?;
            return Ok(None);
        };
        let callee_addr = if let Some(callee) = message.to_header_username() {
            let callee_addr = {
                let system = self.system.lock().await;
                system.registrations.user_addr(&callee)
            };
            if let Some(callee_addr) = callee_addr {
                debug!("route_request: callee \"{}\" is registered", callee);
                callee_addr
            } else {
                debug!("route_request: callee \"{}\" isn't registered", callee);
                self.send_res(&message, 404).await?;
                return Ok(None);
            }
        } else {
            error!("route_request: no `username` in `To`");
            self.send_res(&message, 400).await?;
            return Ok(None);
        };
        // The server should have different dialogs with clients if the server operates in Back-to-Back User Agent mode
        if !self.back_to_back || self.convert_request_dialog(&mut message).await? {
            Ok(Some((callee_addr, message)))
        } else {
            self.send_res(&message, 400).await?;
            Ok(None)
        }
    }

    async fn on_routed_request(&mut self, mut message: SipMessage) -> Result<()> {
        // It's checked in route_request
        let via_branch = message.via_header_branch().unwrap().clone();
        if let Some(h) = message.via_header_mut() {
            *h = self.via_hdr_with_branch(via_branch.clone());
        }
        if self.back_to_back {
            // TODO: This should be checked in route_request
            if let Some(h) = message.contact_header_mut() {
                *h = self.contact_hdr();
            } else {
                // method() is checked to exist in on_message (on_routed_message is called only if some function called from on_message asked the message to be routed)
                if message.method().unwrap() == Method::Invite {
                    error!("on_routed_request: no `Contact` in invite");
                    return Ok(());
                }
            }
        }
        self.sender
            .send(ClientEvent::Send(self.addr, message))
            .await?;
        Ok(())
    }

    async fn on_routed_response(&mut self, mut message: SipMessage) -> Result<()> {
        if self.back_to_back {
            if self.convert_response_dialog(&mut message).await {
                if let Some(h) = message.contact_header_mut() {
                    *h = self.contact_hdr();
                } else {
                    // method() is checked to exist in on_message (on_routed_response is called only if some function called from on_message asked the message to be routed)
                    if message.method().unwrap() == Method::Invite {
                        if let Some(status_code) = message.status_code() {
                            if status_code >= 200 && status_code <= 299 {
                                error!("on_routed_response: no `Contact` in invite 2xx response");
                                return Ok(());
                            }
                        } else {
                            error!("on_routed_response: no status code in response");
                            return Ok(());
                        }
                    }
                }
            } else {
                error!("on_routed_response: convert_response_dialog failed");
                return Ok(());
            }
        }
        self.sender
            .send(ClientEvent::Send(self.addr, message))
            .await?;
        Ok(())
    }

    async fn route_response(
        &mut self,
        message: SipMessage,
    ) -> Result<Option<(SocketAddr, SipMessage)>> {
        let caller = if let Some(caller) = message.from_header_username() {
            caller
        } else {
            error!("route_response: no `username` in `From`");
            return Ok(None);
        };
        let system = self.system.lock().await;
        if let Some(caller_addr) = system.registrations.user_addr(&caller) {
            debug!("route_response: caller \"{}\" is registered", caller);
            Ok(Some((caller_addr, message)))
        } else {
            debug!("route_response: caller \"{}\" isn't registered", caller);
            Ok(None)
        }
    }

    async fn on_subscribe(&mut self, message: SipMessage) -> Result<()> {
        self.send_res(&message, 200).await?;
        if let SipMessage::Request { uri, headers, .. } = message {
            let uri_username = if let Some(auth) = uri.auth {
                auth.username
            } else {
                error!("on_subscribe: no `username` in uri");
                return Ok(());
            };
            let from_hdr = if let Some(Header::From(from_hdr)) = headers.from() {
                from_hdr
            } else {
                error!("on_subscribe: no `From`");
                return Ok(());
            };
            let to_hdr = if let Some(Header::To(to_hdr)) = headers.to() {
                to_hdr.param("tag", Some("123456"))
            } else {
                error!("on_subscribe: no `To`");
                return Ok(());
            };
            let call_id = if let Some(Header::CallId(call_id)) = headers.call_id() {
                call_id
            } else {
                error!("on_subscribe: no `Call-ID`");
                return Ok(());
            };
            let event = if let Some(Header::Event(event)) = headers.event() {
                event
            } else {
                error!("on_subscribe: no `Event`");
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

    async fn send_to_client(&mut self, message: SipMessage) -> Result<()> {
        let message = ClientEvent::Send(self.addr, message);
        self.sender.send(message).await?;
        Ok(())
    }

    /// Uses `self.system.dialogs` to change the current dialog (`call_id`, `server_tag` and `client_tag`) of the request to its linked dialog
    async fn convert_request_dialog(&mut self, message: &mut SipMessage) -> Result<bool> {
        // client_tag is client-created from_tag
        // server_tag is server-created to_tag
        let (call_id, server_tag, client_tag) = {
            let from_tag = if let Some(from_tag) = message.from_header_tag() {
                from_tag
            } else {
                error!("convert_request_dialog: no `tag` in `From`");
                return Ok(false);
            };
            let call_id = if let Some(call_id) = message.call_id() {
                call_id
            } else {
                error!("convert_request_dialog: no `Call-ID`");
                return Ok(false);
            };
            (
                call_id.clone(),
                message.to_header_tag().map(Clone::clone),
                from_tag.clone(),
            )
        };
        if let Some(server_tag) = server_tag {
            let system = self.system.lock().await;
            if let Some(dialog) = system
                .dialogs
                .linked_dialog(&call_id, &server_tag, &client_tag)
            {
                *message.call_id_mut().unwrap() = dialog.call_id().clone();
                message.set_from_header_tag(dialog.server_tag().clone());
                message.set_to_header_tag(dialog.client_tag().clone());
            }
        } else {
            // Generate a server tag
            let server_tag = "toabcd1234".to_string();
            // This request will be sent to another client.
            // This server_tag is server-created from_tag
            let next_dialog_server_tag = "fromabcd1234".to_string();
            message.set_from_header_tag(next_dialog_server_tag.clone());
            let new_call_id = "cidabcd1234".to_string();
            *message.call_id_mut().unwrap() = new_call_id.clone();
            let incomplete_dialog = IncompleteDialogInfo::new(new_call_id, next_dialog_server_tag);
            let dialog = DialogInfo::new(call_id.clone(), server_tag, client_tag.clone());
            let mut system = self.system.lock().await;
            system.dialogs.add(dialog, incomplete_dialog);
        }
        Ok(true)
    }

    async fn convert_response_dialog(&mut self, message: &mut SipMessage) -> bool {
        let (call_id, server_tag, client_tag) = {
            let (call_id, server_tag, client_tag) = {
                let server_tag = if let Some(server_tag) = message.from_header_tag() {
                    server_tag
                } else {
                    error!("convert_response_dialog: no `tag` in `From`");
                    return false;
                };
                let client_tag = if let Some(client_tag) = message.to_header_tag() {
                    client_tag
                } else {
                    error!("convert_response_dialog: no `tag` in `To`");
                    return false;
                };
                let call_id = if let Some(call_id) = message.call_id() {
                    call_id
                } else {
                    error!("convert_response_dialog: no `Call-ID`");
                    return false;
                };
                (call_id, server_tag, client_tag)
            };
            let mut system = self.system.lock().await;
            if let Some(incomplete_dialog) =
                system.dialogs.take_incomplete_dialog(call_id, server_tag)
            {
                system
                    .dialogs
                    .complete_dialog(incomplete_dialog, client_tag.clone());
            }
            if let Some(dialog) = system
                .dialogs
                .linked_dialog(call_id, server_tag, client_tag)
            {
                (
                    dialog.call_id().clone(),
                    dialog.server_tag().clone(),
                    dialog.client_tag().clone(),
                )
            } else {
                error!("convert_response_dialog: no linked dialog");
                return false;
            }
        };
        *message.call_id_mut().unwrap() = call_id;
        message.set_from_header_tag(client_tag);
        message.set_to_header_tag(server_tag);
        true
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

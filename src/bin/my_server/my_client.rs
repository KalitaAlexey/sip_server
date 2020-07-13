use crate::my_system::MySystem;
use async_std::net::SocketAddr;
use async_trait::async_trait;
use libsip::{
    Domain, Header, Method, NamedHeader, RegisterRequestExt, RequestGenerator, ResponseGenerator,
    SipMessage, SipMessageExt, SubscriptionState, Transport, Uri, UriAuth, UriParam, UriSchema,
    ViaHeader,
};
use log::{debug, error};
use sip_server::{
    Client, ClientEvent, ClientEventHandler, DialogInfo, IncompleteDialogInfo, Utils,
};
use std::{collections::HashMap, sync::Arc};

pub struct MyClient<'a> {
    address: SocketAddr,
    transport: Transport,
    schema: UriSchema,
    domain: Domain,
    utils: Arc<Utils>,
    event_handler: Box<dyn ClientEventHandler + 'a>,
    system: Arc<MySystem>,
    back_to_back: bool,
}

#[async_trait]
impl<'a> Client for MyClient<'a> {
    async fn on_msg(&mut self, msg: SipMessage) {
        let method = if let Some(method) = msg.method() {
            method
        } else {
            error!("MyClient::on_message: no method");
            return;
        };
        if msg.is_request() {
            match method {
                Method::Register => {
                    self.on_register(msg).await;
                }
                Method::Subscribe => {
                    self.on_subscribe(msg).await;
                }
                Method::Invite
                | Method::Bye
                | Method::Ack
                | Method::Cancel
                | Method::Refer
                | Method::Notify => {
                    self.route_request(msg).await;
                }
                _ => {
                    self.on_req(msg).await;
                }
            }
        } else {
            match method {
                Method::Invite | Method::Bye | Method::Cancel | Method::Refer | Method::Notify => {
                    self.route_response(msg).await;
                }
                _ => {}
            }
        }
    }

    async fn on_routed_msg(&mut self, msg: SipMessage) {
        if msg.is_request() {
            self.on_routed_request(msg).await;
        } else {
            self.on_routed_response(msg).await;
        }
    }
}

impl<'a> MyClient<'a> {
    pub fn new(
        address: SocketAddr,
        transport: Transport,
        schema: UriSchema,
        domain: Domain,
        utils: Arc<Utils>,
        event_handler: Box<dyn ClientEventHandler + 'a>,
        system: Arc<MySystem>,
        back_to_back: bool,
    ) -> Self {
        Self {
            address,
            transport,
            schema,
            domain,
            utils,
            event_handler,
            system,
            back_to_back,
        }
    }

    async fn on_req(&mut self, msg: SipMessage) {
        self.send_res(&msg, 200).await;
    }

    async fn on_register(&mut self, msg: SipMessage) {
        let username = if let Some(username) = msg.to_header_username() {
            username
        } else {
            error!("on_register: no `username` in `To`");
            self.send_res(&msg, 400).await;
            return;
        };
        let expires = if let Some(expires) = msg.expires() {
            expires
        } else {
            error!("on_register: no `Expires`");
            self.send_res(&msg, 400).await;
            return;
        };
        self.prepare_and_send_res(&msg, 200, |generator| {
            generator.header(Header::Expires(expires))
        })
        .await;
        let mut reg = self.system.registrations.lock().await;
        if expires > 0 {
            reg.register_user(username.clone(), self.address);
        } else {
            reg.unregister_user(&username);
        }
    }

    async fn route_request(&mut self, mut msg: SipMessage) {
        if msg.via_header_branch().is_none() {
            error!("route_request: no `branch` in `Via`");
            self.send_res(&msg, 400).await;
            return;
        };
        let callee_addr = if let Some(callee) = msg.to_header_username() {
            let callee_addr = self.system.registrations.lock().await.user_addr(&callee);
            if let Some(callee_addr) = callee_addr {
                debug!("route_request: callee \"{}\" is registered", callee);
                callee_addr
            } else {
                debug!("route_request: callee \"{}\" isn't registered", callee);
                self.send_res(&msg, 404).await;
                return;
            }
        } else {
            error!("route_request: no `username` in `To`");
            self.send_res(&msg, 400).await;
            return;
        };
        // The server should have different dialogs with clients if the server operates in Back-to-Back User Agent mode
        if !self.back_to_back || self.convert_request_dialog(&mut msg).await {
            self.event_handler
                .handle(ClientEvent::Route {
                    addr: callee_addr,
                    msg,
                })
                .await;
        } else {
            self.send_res(&msg, 400).await;
        }
    }

    async fn on_routed_request(&mut self, mut msg: SipMessage) {
        // It's checked in route_request
        let via_branch = msg.via_header_branch().unwrap().clone();
        if let Some(h) = msg.via_header_mut() {
            *h = self.via_hdr_with_branch(via_branch.clone());
        }
        if self.back_to_back {
            // TODO: This should be checked in route_request
            if let Some(h) = msg.contact_header_mut() {
                *h = self.contact_hdr();
            } else {
                // method() is checked to exist in on_message (on_routed_message is called only if some function called from on_message asked the msg to be routed)
                if msg.method().unwrap() == Method::Invite {
                    error!("on_routed_request: no `Contact` in invite");
                    return;
                }
            }
        }
        self.event_handler.handle(ClientEvent::Send(msg)).await;
    }

    async fn on_routed_response(&mut self, mut msg: SipMessage) {
        if self.back_to_back {
            if self.convert_response_dialog(&mut msg).await {
                if let Some(h) = msg.contact_header_mut() {
                    *h = self.contact_hdr();
                } else {
                    // method() is checked to exist in on_message (on_routed_response is called only if some function called from on_message asked the msg to be routed)
                    if msg.method().unwrap() == Method::Invite {
                        if let Some(status_code) = msg.status_code() {
                            if status_code >= 200 && status_code <= 299 {
                                error!("on_routed_response: no `Contact` in invite 2xx response");
                                return;
                            }
                        } else {
                            error!("on_routed_response: no status code in response");
                            return;
                        }
                    }
                }
            } else {
                error!("on_routed_response: convert_response_dialog failed");
                return;
            }
        }
        self.event_handler.handle(ClientEvent::Send(msg)).await;
    }

    async fn route_response(&mut self, msg: SipMessage) {
        let caller = if let Some(caller) = msg.from_header_username() {
            caller
        } else {
            error!("route_response: no `username` in `From`");
            return;
        };
        if let Some(caller_addr) = self.system.registrations.lock().await.user_addr(&caller) {
            debug!("route_response: caller \"{}\" is registered", caller);
            let event = ClientEvent::Route {
                addr: caller_addr,
                msg,
            };
            self.event_handler.handle(event).await;
        } else {
            debug!("route_response: caller \"{}\" isn't registered", caller);
        }
    }

    async fn on_subscribe(&mut self, msg: SipMessage) {
        self.send_res(&msg, 200).await;
        let (uri, headers) = if let SipMessage::Request { uri, headers, .. } = msg {
            (uri, headers)
        } else {
            error!("on_subscribe: not request");
            return;
        };
        let uri_username = if let Some(auth) = uri.auth {
            auth.username
        } else {
            error!("on_subscribe: no `username` in uri");
            return;
        };
        let from_hdr = if let Some(Header::From(from_hdr)) = headers.from() {
            from_hdr
        } else {
            error!("on_subscribe: no `From`");
            return;
        };
        let to_hdr = if let Some(Header::To(to_hdr)) = headers.to() {
            to_hdr.param("tag", Some("123456"))
        } else {
            error!("on_subscribe: no `To`");
            return;
        };
        let call_id = if let Some(Header::CallId(call_id)) = headers.call_id() {
            call_id
        } else {
            error!("on_subscribe: no `Call-ID`");
            return;
        };
        let event = if let Some(Header::Event(event)) = headers.event() {
            event
        } else {
            error!("on_subscribe: no `Event`");
            return;
        };
        let uri = Uri::new(self.schema, self.domain.clone());
        let uri = uri.auth(UriAuth::new(uri_username));
        match RequestGenerator::new()
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
        {
            Ok(request) => {
                self.send_to_client(request).await;
            }
            Err(e) => error!("on_subscribe: failed to generate notify: {}", e),
        }
    }

    async fn prepare_and_send_res<F>(&mut self, req: &SipMessage, code: u32, f: F)
    where
        F: FnOnce(ResponseGenerator) -> ResponseGenerator,
    {
        let generator = self.create_response_generator(req, code);
        let generator = f(generator);
        match generator.build() {
            Ok(response) => {
                self.send_to_client(response).await;
            }
            Err(e) => error!("prepare_and_send_res: failed to generate response: {}", e),
        }
    }

    async fn send_res(&mut self, req: &SipMessage, code: u32) {
        match self.create_response_generator(req, code).build() {
            Ok(res) => {
                self.send_to_client(res).await;
            }
            Err(e) => error!("send_res: failed to generate response: {}", e),
        }
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
                // 20.14 Content-Length "If no body is present in a msg, then the Content-Length header field value MUST be set to zero"
                .header(Header::ContentLength(0))
        } else {
            panic!("not request");
        }
    }

    async fn send_to_client(&mut self, msg: SipMessage) {
        self.event_handler.handle(ClientEvent::Send(msg)).await;
    }

    /// Uses `self.system.dialogs` to change the current dialog (`call_id`, `server_tag` and `client_tag`) of the request to its linked dialog
    async fn convert_request_dialog(&mut self, msg: &mut SipMessage) -> bool {
        // client_tag is client-created from_tag
        // server_tag is server-created to_tag
        let (call_id, server_tag, client_tag) = {
            let from_tag = if let Some(from_tag) = msg.from_header_tag() {
                from_tag
            } else {
                error!("convert_request_dialog: no `tag` in `From`");
                return false;
            };
            let call_id = if let Some(call_id) = msg.call_id() {
                call_id
            } else {
                error!("convert_request_dialog: no `Call-ID`");
                return false;
            };
            (
                call_id.clone(),
                msg.to_header_tag().map(Clone::clone),
                from_tag.clone(),
            )
        };
        if let Some(server_tag) = server_tag {
            if let Some(dialog) =
                self.system
                    .dialogs
                    .lock()
                    .await
                    .linked_dialog(&call_id, &server_tag, &client_tag)
            {
                *msg.call_id_mut().unwrap() = dialog.call_id().clone();
                msg.set_from_header_tag(dialog.server_tag().clone());
                msg.set_to_header_tag(dialog.client_tag().clone());
            }
        } else {
            // Generate a server tag
            let server_tag = self.system.dialog_gen.tag();
            // This request will be sent to another client.
            // This server_tag is server-created from_tag
            let next_dialog_server_tag = self.system.dialog_gen.tag();
            msg.set_from_header_tag(next_dialog_server_tag.clone());
            let new_call_id = self.system.dialog_gen.call_id();
            *msg.call_id_mut().unwrap() = new_call_id.clone();
            let incomplete_dialog = IncompleteDialogInfo::new(new_call_id, next_dialog_server_tag);
            let dialog = DialogInfo::new(call_id.clone(), server_tag, client_tag.clone());
            self.system
                .dialogs
                .lock()
                .await
                .add(dialog, incomplete_dialog);
        }
        true
    }

    async fn convert_response_dialog(&mut self, msg: &mut SipMessage) -> bool {
        let (call_id, server_tag, client_tag) = {
            let (call_id, server_tag, client_tag) = {
                let server_tag = if let Some(server_tag) = msg.from_header_tag() {
                    server_tag
                } else {
                    error!("convert_response_dialog: no `tag` in `From`");
                    return false;
                };
                let client_tag = if let Some(client_tag) = msg.to_header_tag() {
                    client_tag
                } else {
                    error!("convert_response_dialog: no `tag` in `To`");
                    return false;
                };
                let call_id = if let Some(call_id) = msg.call_id() {
                    call_id
                } else {
                    error!("convert_response_dialog: no `Call-ID`");
                    return false;
                };
                (call_id, server_tag, client_tag)
            };
            let mut dialogs = self.system.dialogs.lock().await;
            if let Some(incomplete_dialog) = dialogs.take_incomplete_dialog(call_id, server_tag) {
                dialogs.complete_dialog(incomplete_dialog, client_tag.clone());
            }
            if let Some(dialog) = dialogs.linked_dialog(call_id, server_tag, client_tag) {
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
        *msg.call_id_mut().unwrap() = call_id;
        msg.set_from_header_tag(client_tag);
        msg.set_to_header_tag(server_tag);
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

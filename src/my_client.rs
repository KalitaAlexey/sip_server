use crate::{
    my_system::MySystem,
    utils::{self, Utils},
};
use crate::{Client, ClientEvent, Result, Sender};
use async_std::{net::SocketAddr, sync::Mutex, task};
use async_trait::async_trait;
use futures::sink::SinkExt;
use libsip::{
    Domain, Header, Method, NamedHeader, RequestGenerator, ResponseGenerator, SipMessage,
    Transport, Uri, UriAuth, UriParam, UriSchema, ViaHeader,
};
use log::{debug, info, warn};
use std::{str::FromStr, sync::Arc};

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
    async fn on_message(&mut self, msg: SipMessage) -> Result<()> {
        let (request, method) = match &msg {
            SipMessage::Request { method, .. } => (true, *method),
            SipMessage::Response { headers, .. } => {
                if let Some(Header::CSeq(_, method)) = headers.cseq() {
                    (false, method)
                } else {
                    warn!("no `CSeq` in response");
                    return Ok(());
                }
            }
        };
        if request {
            match method {
                Method::Register => self.on_register(msg).await,
                Method::Subscribe => self.on_subscribe(msg).await,
                Method::Invite | Method::Bye | Method::Ack | Method::Cancel => {
                    self.route_request(msg).await
                }
                _ => self.on_req(msg).await,
            }
        } else {
            match method {
                Method::Invite | Method::Bye | Method::Cancel => self.route_response(msg).await,
                _ => Ok(()),
            }
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
        let username = if let Some(username) = Self::get_to_username(&msg) {
            username
        } else {
            warn!("on_register: no `username` in `To`");
            return self.send_res(&msg, 400).await;
        };
        let expires = if let Some(expires) = Self::get_expires(&msg) {
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
            if system.add_registration(username.clone(), self.addr) {
                info!("user \"{}\" is registered", username);
            }
        } else if system.remove_registration(&username) {
            info!("user \"{}\" is unregistered", username);
        }
        Ok(())
    }

    async fn route_request(&mut self, mut msg: SipMessage) -> Result<()> {
        let callee = if let Some(callee) = Self::get_to_username(&msg) {
            callee
        } else {
            warn!("route_request: no `username` in `To`");
            return Ok(());
        };
        let callee_addr = {
            let system = self.system.lock().await;
            system.get_registration(&callee).map(Clone::clone)
        };
        let callee_addr = if let Some(callee_addr) = callee_addr {
            warn!("route_request: callee \"{}\" is registered", callee);
            callee_addr
        } else {
            warn!("route_request: callee \"{}\" isn't registered", callee);
            return self.send_res(&msg, 404).await;
        };
        let via_branch = if let Some(via_branch) = Self::get_via_branch(&msg) {
            via_branch
        } else {
            warn!("route_request: no `branch` in `Via`");
            return self.send_res(&msg, 400).await;
        };
        let h =
            msg.headers_mut().0.iter_mut().find(
                |h| {
                    if let Header::Via(_) = h {
                        true
                    } else {
                        false
                    }
                },
            );
        if let Some(h) = h {
            *h = self.via_hdr_with_branch(via_branch);
        }
        let h = msg.headers_mut().0.iter_mut().find(|h| {
            if let Header::Contact(_) = h {
                true
            } else {
                false
            }
        });
        if let Some(h) = h {
            *h = self.contact_hdr();
        }
        self.sender
            .send(ClientEvent::Send(callee_addr, msg))
            .await?;
        Ok(())
    }

    async fn route_response(&mut self, msg: SipMessage) -> Result<()> {
        let caller = if let Some(caller) = Self::from_username(&msg) {
            caller
        } else {
            warn!("route_response: no `username` in `From`");
            return Ok(());
        };
        let caller_addr = {
            let system = self.system.lock().await;
            system.get_registration(&caller).map(Clone::clone)
        };
        if let Some(caller_addr) = caller_addr {
            debug!("route_response: caller \"{}\" is registered", caller);
            self.sender
                .send(ClientEvent::Send(caller_addr, msg))
                .await?;
            Ok(())
        } else {
            debug!("route_response: caller \"{}\" isn't registered", caller);
            Ok(())
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
                .header(Header::ContentLength(0))
                .build()
                .expect("failed to generate notify");
            let msg = ClientEvent::Send(self.addr, req);
            let sender = self.sender.clone();
            // Maybe it should be saved so it can be waited for before shutdown
            task::spawn(utils::delay_and_send_msg(sender, 2, msg));
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
                .header(self.contact_hdr())
                // 20.14 Content-Length "If no body is present in a message, then the Content-Length header field value MUST be set to zero"
                .header(Header::ContentLength(0))
        } else {
            panic!("not request");
        }
    }

    fn from_username(msg: &SipMessage) -> Option<String> {
        if let Some(Header::From(hdr)) = msg.headers().from() {
            if let Some(auth) = hdr.uri.auth {
                Some(auth.username)
            } else {
                None
            }
        } else {
            None
        }
    }

    fn get_to_username(msg: &SipMessage) -> Option<String> {
        if let Some(Header::To(hdr)) = msg.headers().to() {
            if let Some(auth) = hdr.uri.auth {
                Some(auth.username)
            } else {
                None
            }
        } else {
            None
        }
    }

    async fn send_to_client(&mut self, msg: SipMessage) -> Result<()> {
        let msg = ClientEvent::Send(self.addr, msg);
        self.sender.send(msg).await?;
        Ok(())
    }

    fn contact_hdr(&self) -> Header {
        Header::Contact(NamedHeader::new(Uri::new(self.schema, self.domain.clone())))
    }

    async fn via_hdr(&self) -> Header {
        let via_branch = self.utils.via_branch().await;
        self.via_hdr_with_branch(via_branch)
    }

    fn via_hdr_with_branch(&self, branch: String) -> Header {
        let via_uri = Uri::new_schemaless(self.domain.clone());
        let via_uri = via_uri.parameter(UriParam::Branch(branch));
        Header::Via(ViaHeader::new(via_uri, self.transport))
    }

    fn get_via_branch(msg: &SipMessage) -> Option<String> {
        if let Some(Header::Via(hdr)) = msg.headers().via() {
            hdr.uri.parameters.iter().find_map(|p| {
                if let UriParam::Branch(b) = p {
                    Some(b.clone())
                } else {
                    None
                }
            })
        } else {
            None
        }
    }

    fn get_expires(msg: &SipMessage) -> Option<u32> {
        let headers = msg.headers();
        let expires = if let Some(Header::Contact(hdr)) = headers.contact() {
            hdr.parameters.get("expires").and_then(|expires| {
                if let Some(expires) = expires {
                    u32::from_str(expires).ok()
                } else {
                    None
                }
            })
        } else {
            None
        };
        if expires.is_some() {
            expires
        } else if let Some(Header::Expires(expires)) = headers.expires() {
            Some(expires)
        } else {
            None
        }
    }
}

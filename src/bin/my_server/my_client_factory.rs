use crate::{my_client::MyClient, my_system::MySystem};
use async_std::{net::SocketAddr, sync::Mutex};
use libsip::{Domain, Transport, UriSchema};
use sip_server::{Client, ClientEventHandler, ClientFactory, Utils};
use std::sync::Arc;

pub struct MyClientFactory {
    transport: Transport,
    schema: UriSchema,
    domain: Domain,
    utils: Arc<Utils>,
    system: Arc<Mutex<MySystem>>,
    back_to_back: bool,
}

impl ClientFactory for MyClientFactory {
    fn create_client<'a>(
        &self,
        address: SocketAddr,
        event_handler: Box<dyn ClientEventHandler + 'a>,
    ) -> Box<dyn Client + 'a> {
        Box::new(MyClient::new(
            address,
            self.transport,
            self.schema,
            self.domain.clone(),
            self.utils.clone(),
            event_handler,
            self.system.clone(),
            self.back_to_back,
        ))
    }
}

impl MyClientFactory {
    pub fn new(
        transport: Transport,
        schema: UriSchema,
        domain: Domain,
        back_to_back: bool,
    ) -> Self {
        Self {
            transport,
            schema,
            domain,
            system: Arc::new(Mutex::new(MySystem::new())),
            utils: Arc::new(Utils::new()),
            back_to_back,
        }
    }
}

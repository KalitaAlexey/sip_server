use crate::{my_client::MyClient, my_system::MySystem};
use async_std::{net::SocketAddr, sync::Mutex};
use libsip::{Domain, Transport, UriSchema};
use sip_server::{ClientEventHandler, ClientManager, Utils};
use std::{
    collections::{hash_map::Entry, HashMap},
    sync::Arc,
};

pub struct MyClientManager {
    transport: Transport,
    schema: UriSchema,
    domain: Domain,
    clients: HashMap<SocketAddr, MyClient>,
    utils: Arc<Utils>,
    system: Arc<Mutex<MySystem>>,
    back_to_back: bool,
}

impl ClientManager for MyClientManager {
    type Client = MyClient;

    fn create_client(
        &mut self,
        address: SocketAddr,
        event_handler: Box<dyn ClientEventHandler>,
    ) -> &mut Self::Client {
        match self.clients.entry(address) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => {
                let client = MyClient::new(
                    address,
                    self.transport,
                    self.schema,
                    self.domain.clone(),
                    self.utils.clone(),
                    event_handler,
                    self.system.clone(),
                    self.back_to_back,
                );
                entry.insert(client)
            }
        }
    }

    fn get_client(&mut self, addr: &SocketAddr) -> Option<&mut Self::Client> {
        self.clients.get_mut(addr)
    }
}

impl MyClientManager {
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
            clients: HashMap::new(),
            system: Arc::new(Mutex::new(MySystem::new())),
            utils: Arc::new(Utils::new()),
            back_to_back,
        }
    }
}

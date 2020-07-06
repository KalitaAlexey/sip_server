use crate::{my_client::MyClient, my_system::MySystem};
use async_std::{net::SocketAddr, sync::Mutex};
use libsip::{Domain, Transport, UriSchema};
use sip_server::{ClientEvent, ClientManager, Sender, Utils};
use std::{
    collections::{hash_map::Entry, HashMap},
    sync::Arc,
};

pub struct MyClientManager {
    transport: Transport,
    schema: UriSchema,
    domain: Domain,
    sender: Sender<ClientEvent>,
    clients: HashMap<SocketAddr, MyClient>,
    utils: Arc<Utils>,
    system: Arc<Mutex<MySystem>>,
    back_to_back: bool,
}

impl ClientManager for MyClientManager {
    type Client = MyClient;

    fn get_client(&mut self, addr: SocketAddr) -> &mut Self::Client {
        match self.clients.entry(addr) {
            Entry::Vacant(entry) => {
                let h = MyClient::new(
                    addr,
                    self.transport,
                    self.schema,
                    self.domain.clone(),
                    self.utils.clone(),
                    self.sender.clone(),
                    self.system.clone(),
                    self.back_to_back,
                );
                entry.insert(h)
            }
            Entry::Occupied(entry) => entry.into_mut(),
        }
    }
}

impl MyClientManager {
    pub fn new(
        transport: Transport,
        schema: UriSchema,
        domain: Domain,
        sender: Sender<ClientEvent>,
        back_to_back: bool,
    ) -> Self {
        Self {
            transport,
            schema,
            domain,
            sender,
            clients: HashMap::new(),
            system: Arc::new(Mutex::new(MySystem::new())),
            utils: Arc::new(Utils::new()),
            back_to_back,
        }
    }
}

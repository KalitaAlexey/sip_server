use async_std::{net::SocketAddr, sync::Mutex};
use libsip::{Domain, Transport, UriSchema};
use std::{
    collections::{hash_map::Entry, HashMap},
    sync::Arc,
};

use crate::{
    client_handler::ClientHandlerMsg, client_handler_mgr::ClientHandlerMgr,
    my_client_handler::MyClientHandler, my_system::MySystem, utils::Utils, Sender,
};

pub struct MyClientHandlerMgr {
    transport: Transport,
    schema: UriSchema,
    domain: Domain,
    sender: Sender<ClientHandlerMsg>,
    handlers: HashMap<SocketAddr, MyClientHandler>,
    utils: Arc<Utils>,
    system: Arc<Mutex<MySystem>>,
}

impl ClientHandlerMgr for MyClientHandlerMgr {
    type Handler = MyClientHandler;

    fn get_handler(&mut self, addr: SocketAddr) -> &mut Self::Handler {
        match self.handlers.entry(addr) {
            Entry::Vacant(entry) => {
                let h = MyClientHandler::new(
                    addr,
                    self.transport,
                    self.schema,
                    self.domain.clone(),
                    self.utils.clone(),
                    self.sender.clone(),
                    self.system.clone(),
                );
                entry.insert(h)
            }
            Entry::Occupied(entry) => entry.into_mut(),
        }
    }
}

impl MyClientHandlerMgr {
    pub fn new(
        transport: Transport,
        schema: UriSchema,
        domain: Domain,
        sender: Sender<ClientHandlerMsg>,
    ) -> Self {
        Self {
            transport,
            schema,
            domain,
            sender,
            handlers: HashMap::new(),
            system: Arc::new(Mutex::new(MySystem::new())),
            utils: Arc::new(Utils::new()),
        }
    }
}

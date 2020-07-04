use async_std::net::SocketAddr;
use std::collections::HashMap;

pub struct MySystem {
    registrations: HashMap<String, SocketAddr>,
}

impl MySystem {
    pub fn new() -> Self {
        Self {
            registrations: HashMap::new(),
        }
    }

    pub fn add_registration(&mut self, user: String, addr: SocketAddr) -> bool {
        self.registrations.insert(user, addr).is_none()
    }

    pub fn get_registration(&self, user: &str) -> Option<&SocketAddr> {
        self.registrations.get(user)
    }

    pub fn remove_registration(&mut self, user: &str) -> bool {
        self.registrations.remove(user).is_some()
    }
}

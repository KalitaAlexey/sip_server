use log::info;
use std::{
    collections::{hash_map::Entry, HashMap},
    net::SocketAddr,
};

/// Manages registrations
/// # Examples
/// ```
/// use sip_server::Registrations;
///
/// let mut registrations = Registrations::new();
/// let user = "joe";
/// let addr = "192.168.0.50:44374".parse().expect("failed to parse socket addr");
///
/// assert!(registrations.register_user(user.to_string(), addr));
///
/// assert!(!registrations.register_user(user.to_string(), addr));
///
/// assert_eq!(registrations.user_addr(user), Some(addr));
///
/// assert!(registrations.unregister_user(user));
///
/// assert_eq!(registrations.user_addr(user), None);
/// ```
#[derive(Clone, Default, Debug)]
pub struct Registrations(HashMap<String, SocketAddr>);

impl Registrations {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a new user or updates an existing user's address.
    /// Returns `true` if `user` is new
    pub fn register_user(&mut self, user: String, addr: SocketAddr) -> bool {
        match self.0.entry(user) {
            Entry::Occupied(mut entry) => {
                let addr_changed = {
                    let user = entry.key();
                    let current_addr = entry.get();
                    if *current_addr != addr {
                        info!(
                            "user \"{}\" address is changed: {} -> {}",
                            user, current_addr, addr
                        );
                        true
                    } else {
                        false
                    }
                };
                if addr_changed {
                    *entry.get_mut() = addr;
                }
                false
            }
            Entry::Vacant(entry) => {
                info!("user \"{}\" is registered: {}", entry.key(), addr);
                entry.insert(addr);
                true
            }
        }
    }

    /// Returns `true` if `user` was registered, but is no longer registered
    pub fn unregister_user(&mut self, user: &str) -> bool {
        if self.0.remove(user).is_some() {
            info!("user \"{}\" is unregistered", user);
            true
        } else {
            false
        }
    }

    /// Returns a given user's address
    pub fn user_addr(&self, user: &str) -> Option<SocketAddr> {
        self.0.get(user).map(Clone::clone)
    }
}

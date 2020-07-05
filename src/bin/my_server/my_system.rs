use sip_server::{Dialogs, Registrations};

#[derive(Debug, Default)]
pub struct MySystem {
    pub dialogs: Dialogs,
    pub registrations: Registrations,
}

impl MySystem {
    pub fn new() -> Self {
        Self {
            dialogs: Dialogs::new(),
            registrations: Registrations::new(),
        }
    }
}

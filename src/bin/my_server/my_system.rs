use async_std::sync::Mutex;
use sip_server::{DialogGen, Dialogs, Registrations};

#[derive(Debug)]
pub struct MySystem {
    pub dialogs: Mutex<Dialogs>,
    pub registrations: Mutex<Registrations>,
    pub dialog_gen: DialogGen,
}

impl MySystem {
    pub fn new() -> Self {
        Self {
            dialogs: Mutex::new(Dialogs::new()),
            registrations: Mutex::new(Registrations::new()),
            dialog_gen: DialogGen::new(),
        }
    }
}

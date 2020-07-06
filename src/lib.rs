#![recursion_limit = "1024"]

use futures::channel::mpsc;

mod client;
mod client_event;
mod client_event_handler;
mod client_manager;
mod components;
mod server;
mod udp_client_event_handler;
mod utils;
mod via_branch_generator;

pub use self::{
    client::Client, client_event::ClientEvent, client_event_handler::ClientEventHandler,
    client_manager::ClientManager, components::*, server::Server, utils::Utils,
    via_branch_generator::ViaBranchGenerator,
};

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
pub type Sender<T> = mpsc::UnboundedSender<T>;
pub type Receiver<T> = mpsc::UnboundedReceiver<T>;

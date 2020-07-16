#![recursion_limit = "1024"]

use futures::channel::mpsc;

mod client;
mod client_worker;
mod components;
mod message_router;
mod server;
mod sip_parse;
mod tcp_server;
mod udp_server;
mod utils;
mod via_branch_generator;

pub use self::{
    client::{Client, ClientEvent, ClientEventHandler, ClientFactory},
    components::*,
    server::Server,
    utils::Utils,
    via_branch_generator::ViaBranchGenerator,
};

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
pub type Sender<T> = mpsc::UnboundedSender<T>;
pub type Receiver<T> = mpsc::UnboundedReceiver<T>;

// TODO: Remove all expect and unwrap

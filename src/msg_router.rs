use crate::{client_worker::ClientWorkerMessage, Receiver, Sender};
use async_std::net::SocketAddr;
use futures::{SinkExt, StreamExt};
use libsip::SipMessage;
use log::error;
use std::collections::{hash_map::Entry, HashMap};

/// Message handled by MsgRouter
pub(crate) enum MsgRouterMsg {
    /// New client worker created
    ClientWorker {
        addr: SocketAddr,
        sender: Sender<ClientWorkerMessage>,
    },
    /// Message routed to be handled by another client worker
    RoutedMessage { addr: SocketAddr, msg: SipMessage },
}

/// Reads incoming messages and routes them to matching clients
pub(crate) struct MsgRouter {
    receiver: Receiver<MsgRouterMsg>,
    senders: HashMap<SocketAddr, Sender<ClientWorkerMessage>>,
}

impl MsgRouter {
    pub fn new(receiver: Receiver<MsgRouterMsg>) -> Self {
        Self {
            receiver,
            senders: HashMap::new(),
        }
    }

    pub async fn run(mut self) {
        while let Some(msg) = self.receiver.next().await {
            match msg {
                MsgRouterMsg::ClientWorker { addr, sender } => self.add_client_worker(addr, sender),
                MsgRouterMsg::RoutedMessage { addr, msg } => {
                    self.send_to_client_worker(addr, ClientWorkerMessage::Routed(msg))
                        .await;
                }
            }
        }
    }

    fn add_client_worker(&mut self, addr: SocketAddr, sender: Sender<ClientWorkerMessage>) {
        match self.senders.entry(addr) {
            Entry::Vacant(entry) => {
                entry.insert(sender);
            }
            Entry::Occupied(_) => error!("{} already has client worker", addr),
        }
    }

    async fn send_to_client_worker(&mut self, addr: SocketAddr, msg: ClientWorkerMessage) {
        if let Some(sender) = self.senders.get_mut(&addr) {
            if let Err(e) = sender.send(msg).await {
                error!("send failed: {}", e);
            }
        } else {
            error!("{} doesn't have client worker", addr);
        }
    }
}

use crate::{client_worker::ClientWorkerMessage, Receiver, Sender};
use async_std::net::SocketAddr;
use futures::{SinkExt, StreamExt};
use libsip::SipMessage;
use log::error;
use std::collections::{hash_map::Entry, HashMap};

/// Message handled by MessageRouter
pub(crate) enum MessageRouterMessage {
    /// New client worker created
    ClientWorker {
        addr: SocketAddr,
        sender: Sender<ClientWorkerMessage>,
    },
    /// Message received from network to be handled by client worker
    ReceivedMessage { addr: SocketAddr, msg: SipMessage },
    /// Message routed to be handled by another client worker
    RoutedMessage { addr: SocketAddr, msg: SipMessage },
}

/// Reads incoming messages and routes them to matching clients
pub(crate) struct MessageRouter {
    receiver: Receiver<MessageRouterMessage>,
    senders: HashMap<SocketAddr, Sender<ClientWorkerMessage>>,
}

impl MessageRouter {
    pub fn new(receiver: Receiver<MessageRouterMessage>) -> Self {
        Self {
            receiver,
            senders: HashMap::new(),
        }
    }

    pub async fn run(mut self) {
        while let Some(msg) = self.receiver.next().await {
            match msg {
                MessageRouterMessage::ClientWorker { addr, sender } => {
                    self.add_client_worker(addr, sender)
                }
                MessageRouterMessage::ReceivedMessage { addr, msg } => {
                    self.send_to_client_worker(addr, ClientWorkerMessage::Received(msg))
                        .await;
                }
                MessageRouterMessage::RoutedMessage { addr, msg } => {
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

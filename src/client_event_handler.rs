use crate::ClientEvent;
use async_trait::async_trait;

#[async_trait]
/// Handles [`ClientEvent`](enum.ClientEvent.html) provided by [`Client`](trait.Client.html)
pub trait ClientEventHandler: Send + Sync {
    async fn handle(&mut self, event: ClientEvent);
}

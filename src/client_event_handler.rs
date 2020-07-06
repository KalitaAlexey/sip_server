use crate::ClientEvent;
use async_trait::async_trait;

#[async_trait]
/// Handles [`ClientEvent`](enum.ClientEvent.html) provided by [`Client`](trait.Client.html)
pub trait ClientEventHandler: Sync + Send {
    async fn handle(&mut self, event: ClientEvent);
}

use tokio::sync::broadcast;
use crate::protocol::message::CdpEvent;

pub type EventSender = broadcast::Sender<CdpEvent>;
pub type EventReceiver = broadcast::Receiver<CdpEvent>;

/// Global event bus for broadcasting CDP events to all WebSocket connections.
pub struct EventBus {
    sender: EventSender,
}

impl EventBus {
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }

    pub fn sender(&self) -> EventSender {
        self.sender.clone()
    }

    pub fn subscribe(&self) -> EventReceiver {
        self.sender.subscribe()
    }
}

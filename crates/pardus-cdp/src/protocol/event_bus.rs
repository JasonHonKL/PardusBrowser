use crate::protocol::message::CdpEvent;
use std::collections::VecDeque;
use tokio::sync::broadcast;

pub type EventSender = broadcast::Sender<CdpEvent>;
pub type EventReceiver = broadcast::Receiver<CdpEvent>;

const REPLAY_BUFFER_SIZE: usize = 64;

pub struct EventBus {
    sender: EventSender,
    replay_buffer: std::sync::Mutex<VecDeque<CdpEvent>>,
}

impl EventBus {
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self {
            sender,
            replay_buffer: std::sync::Mutex::new(VecDeque::with_capacity(REPLAY_BUFFER_SIZE)),
        }
    }

    pub fn sender(&self) -> EventSender {
        self.sender.clone()
    }

    pub fn subscribe(&self) -> EventReceiver {
        self.sender.subscribe()
    }

    pub fn send(&self, event: CdpEvent) {
        {
            let mut buffer = self.replay_buffer.lock().unwrap_or_else(|e| e.into_inner());
            if buffer.len() >= REPLAY_BUFFER_SIZE {
                buffer.pop_front();
            }
            buffer.push_back(event.clone());
        }
        let _ = self.sender.send(event);
    }

    pub fn replay_events(&self) -> Vec<CdpEvent> {
        let buffer = self.replay_buffer.lock().unwrap_or_else(|e| e.into_inner());
        buffer.iter().cloned().collect()
    }
}

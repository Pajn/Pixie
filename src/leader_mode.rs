use crossbeam::channel::{Receiver, Sender, unbounded};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use crate::accessibility::Direction;
use crate::config::Action;
use crate::error::Result;

#[derive(Debug, Clone)]
pub enum LeaderModeEvent {
    RegisterSlot(char),
    FocusSlot(char),
    Cancelled,
    KeybindAction(Action),
    FocusDirection(Direction),
}

pub struct LeaderModeController {
    event_receiver: Receiver<LeaderModeEvent>,
    event_sender: Sender<LeaderModeEvent>,
    is_listening: Arc<AtomicBool>,
    timeout: Duration,
}

impl LeaderModeController {
    pub fn new() -> Result<Self> {
        Self::with_timeout(Duration::from_secs(2))
    }

    pub fn with_timeout(timeout: Duration) -> Result<Self> {
        let (event_sender, event_receiver) = unbounded();
        let is_listening = Arc::new(AtomicBool::new(false));

        Ok(LeaderModeController {
            event_receiver,
            event_sender,
            is_listening,
            timeout,
        })
    }

    pub fn enter_listening_mode(&self) {
        self.is_listening.store(true, Ordering::SeqCst);

        let is_listening = Arc::clone(&self.is_listening);
        let sender = self.event_sender.clone();
        let timeout = self.timeout;
        thread::spawn(move || {
            thread::sleep(timeout);
            if is_listening.swap(false, Ordering::SeqCst) {
                let _ = sender.send(LeaderModeEvent::Cancelled);
            }
        });
    }

    pub fn handle_key(&self, key: char, shift: bool) {
        if !self.is_listening.swap(false, Ordering::SeqCst) {
            return;
        }

        let event = if shift {
            LeaderModeEvent::RegisterSlot(key.to_ascii_uppercase())
        } else {
            LeaderModeEvent::FocusSlot(key)
        };
        let _ = self.event_sender.send(event);
    }

    pub fn cancel(&self) {
        if self.is_listening.swap(false, Ordering::SeqCst) {
            let _ = self.event_sender.send(LeaderModeEvent::Cancelled);
        }
    }

    pub fn handle_action(&self, action: Action) {
        self.is_listening.store(false, Ordering::SeqCst);
        let _ = self
            .event_sender
            .send(LeaderModeEvent::KeybindAction(action));
    }

    pub fn handle_direction(&self, direction: Direction) {
        self.is_listening.store(false, Ordering::SeqCst);
        let _ = self
            .event_sender
            .send(LeaderModeEvent::FocusDirection(direction));
    }

    pub fn send_action(&self, action: Action) {
        let _ = self
            .event_sender
            .send(LeaderModeEvent::KeybindAction(action));
    }

    pub fn events(&self) -> Receiver<LeaderModeEvent> {
        self.event_receiver.clone()
    }

    pub fn is_listening(&self) -> bool {
        self.is_listening.load(Ordering::SeqCst)
    }
}

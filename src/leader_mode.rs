use crossbeam::channel::{unbounded, Receiver, Sender};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use rdev::{grab, Event, EventType, Key};

use crate::error::Result;

const LISTEN_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LeaderState {
    Idle,
    Listening,
}

#[derive(Debug, Clone)]
pub enum LeaderModeEvent {
    RegisterSlot(char),
    FocusSlot(char),
    Cancelled,
}

pub struct LeaderModeController {
    is_listening: Arc<AtomicBool>,
    event_receiver: Receiver<LeaderModeEvent>,
    timeout_instant: Arc<std::sync::Mutex<Option<Instant>>>,
}

impl LeaderModeController {
    pub fn new() -> Result<Self> {
        let (event_sender, event_receiver): (Sender<LeaderModeEvent>, Receiver<LeaderModeEvent>) =
            unbounded();
        let is_listening = Arc::new(AtomicBool::new(false));
        let timeout_instant = Arc::new(std::sync::Mutex::new(None::<Instant>));
        let shift_pressed = Arc::new(AtomicBool::new(false));

        let is_listening_clone = Arc::clone(&is_listening);
        let sender_clone = event_sender;
        let timeout_clone = Arc::clone(&timeout_instant);
        let shift_clone = Arc::clone(&shift_pressed);

        thread::spawn(move || {
            let callback = move |event: Event| -> Option<Event> {
                if !is_listening_clone.load(Ordering::SeqCst) {
                    if matches!(
                        event.event_type,
                        EventType::KeyPress(Key::ShiftLeft | Key::ShiftRight)
                    ) {
                        shift_clone.store(true, Ordering::SeqCst);
                    }
                    if matches!(
                        event.event_type,
                        EventType::KeyRelease(Key::ShiftLeft | Key::ShiftRight)
                    ) {
                        shift_clone.store(false, Ordering::SeqCst);
                    }
                    return Some(event);
                }

                if let Some(instant) = timeout_clone.lock().unwrap().as_ref() {
                    if instant.elapsed() > LISTEN_TIMEOUT {
                        is_listening_clone.store(false, Ordering::SeqCst);
                        let _ = sender_clone.send(LeaderModeEvent::Cancelled);
                        return Some(event);
                    }
                }

                match &event.event_type {
                    EventType::KeyPress(Key::ShiftLeft | Key::ShiftRight) => {
                        shift_clone.store(true, Ordering::SeqCst);
                        None
                    }
                    EventType::KeyRelease(Key::ShiftLeft | Key::ShiftRight) => {
                        shift_clone.store(false, Ordering::SeqCst);
                        None
                    }
                    EventType::KeyPress(Key::Escape) => {
                        is_listening_clone.store(false, Ordering::SeqCst);
                        let _ = sender_clone.send(LeaderModeEvent::Cancelled);
                        None
                    }
                    EventType::KeyPress(key) => {
                        if let Some(c) = key_to_char(key) {
                            is_listening_clone.store(false, Ordering::SeqCst);
                            if shift_clone.load(Ordering::SeqCst) {
                                let _ = sender_clone
                                    .send(LeaderModeEvent::RegisterSlot(c.to_ascii_uppercase()));
                            } else {
                                let _ = sender_clone.send(LeaderModeEvent::FocusSlot(c));
                            }
                        } else {
                            is_listening_clone.store(false, Ordering::SeqCst);
                            let _ = sender_clone.send(LeaderModeEvent::Cancelled);
                        }
                        None
                    }
                    _ => None,
                }
            };

            if let Err(e) = grab(callback) {
                eprintln!("Leader mode grab error: {:?}", e);
            }
        });

        Ok(LeaderModeController {
            is_listening,
            event_receiver,
            timeout_instant,
        })
    }

    pub fn enter_listening_mode(&self) {
        self.is_listening.store(true, Ordering::SeqCst);
        *self.timeout_instant.lock().unwrap() = Some(Instant::now());
    }

    pub fn events(&self) -> Receiver<LeaderModeEvent> {
        self.event_receiver.clone()
    }
}

fn key_to_char(key: &Key) -> Option<char> {
    match key {
        Key::KeyA => Some('a'),
        Key::KeyB => Some('b'),
        Key::KeyC => Some('c'),
        Key::KeyD => Some('d'),
        Key::KeyE => Some('e'),
        Key::KeyF => Some('f'),
        Key::KeyG => Some('g'),
        Key::KeyH => Some('h'),
        Key::KeyI => Some('i'),
        Key::KeyJ => Some('j'),
        Key::KeyK => Some('k'),
        Key::KeyL => Some('l'),
        Key::KeyM => Some('m'),
        Key::KeyN => Some('n'),
        Key::KeyO => Some('o'),
        Key::KeyP => Some('p'),
        Key::KeyQ => Some('q'),
        Key::KeyR => Some('r'),
        Key::KeyS => Some('s'),
        Key::KeyT => Some('t'),
        Key::KeyU => Some('u'),
        Key::KeyV => Some('v'),
        Key::KeyW => Some('w'),
        Key::KeyX => Some('x'),
        Key::KeyY => Some('y'),
        Key::KeyZ => Some('z'),
        _ => None,
    }
}

use core_foundation::runloop::{CFRunLoop, kCFRunLoopCommonModes};
use core_graphics::event::{
    CGEventFlags, CGEventTap, CGEventTapLocation, CGEventTapOptions, CGEventTapPlacement,
    CGEventType, EventField,
};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

use crate::config::{Action, KeyCode, Keybind, KeybindEntry, Modifiers};
use crate::ui::{PickerInput, is_window_picker_active, picker_input_from_keycode};

pub static IS_LISTENING: AtomicBool = AtomicBool::new(false);
static LEADER_MODIFIERS_ACTIVE: AtomicBool = AtomicBool::new(false);
static PICKER_REPEAT_COUNTER: AtomicU8 = AtomicU8::new(0);

#[derive(Debug, Clone)]
pub enum EventTapAction {
    LeaderPressed,
    LeaderReleased,
    KeyPressed(i64, bool),
    ActionTriggered(Action),
    ArrowPressed(crate::accessibility::Direction),
    PickerInput(PickerInput),
}

pub struct EventTap {
    runloop: Arc<CFRunLoop>,
}

impl EventTap {
    pub fn new(
        leader_modifiers: Modifiers,
        leader_keycode: KeyCode,
        keybinds: Vec<KeybindEntry>,
        sender: tokio::sync::mpsc::UnboundedSender<EventTapAction>,
    ) -> Result<Self, String> {
        let leader_flags = modifiers_to_cg_flags(leader_modifiers);
        let leader_kc = keycode_to_native(leader_keycode);

        let (ready_tx, ready_rx) = std::sync::mpsc::channel::<Result<Arc<CFRunLoop>, String>>();

        std::thread::Builder::new()
            .name("event_tap".into())
            .spawn(move || {
                let current = CFRunLoop::get_current();
                let current = Arc::new(current);

                let handler = EventHandler {
                    leader_modifiers: leader_flags,
                    leader_keycode: leader_kc,
                    keybinds,
                    sender,
                };

                let tap = match CGEventTap::new(
                    CGEventTapLocation::Session,
                    CGEventTapPlacement::HeadInsertEventTap,
                    CGEventTapOptions::Default,
                    vec![CGEventType::FlagsChanged, CGEventType::KeyDown],
                    move |_, event_type, event| {
                        let keycode = event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE);
                        let is_autorepeat =
                            event.get_integer_value_field(EventField::KEYBOARD_EVENT_AUTOREPEAT) == 1;
                        let flags = event.get_flags();
                        let mut new_event = event.clone();

                        handler.handle_event(
                            event_type,
                            keycode,
                            flags,
                            is_autorepeat,
                            &mut new_event,
                        );

                        Some(new_event)
                    },
                ) {
                    Ok(tap) => tap,
                    Err(e) => {
                        let _ = ready_tx.send(Err(format!(
                            "Failed to create event tap. Make sure Pixie has Accessibility permissions.\n\
                             System Preferences > Privacy & Security > Accessibility > Add Pixie.app\n\
                             Error: {:?}",
                            e
                        )));
                        return;
                    }
                };

                let loop_source = match tap.mach_port.create_runloop_source(0) {
                    Ok(source) => source,
                    Err(e) => {
                        let _ = ready_tx.send(Err(format!("Failed to create runloop source: {:?}", e)));
                        return;
                    }
                };

                unsafe {
                    current.add_source(&loop_source, kCFRunLoopCommonModes);
                    tap.enable();
                }

                let _ = ready_tx.send(Ok(Arc::clone(&current)));

                unsafe {
                    core_foundation::runloop::CFRunLoopRun();
                }
            })
            .map_err(|e| format!("Failed to spawn event tap thread: {:?}", e))?;

        let runloop = ready_rx
            .recv()
            .map_err(|e| format!("Event tap thread crashed: {:?}", e))??;

        Ok(Self { runloop })
    }
}

struct EventHandler {
    leader_modifiers: CGEventFlags,
    leader_keycode: i64,
    keybinds: Vec<KeybindEntry>,
    sender: tokio::sync::mpsc::UnboundedSender<EventTapAction>,
}

impl EventHandler {
    fn handle_event(
        &self,
        event_type: CGEventType,
        keycode: i64,
        flags: CGEventFlags,
        is_autorepeat: bool,
        event: &mut core_graphics::event::CGEvent,
    ) {
        match event_type {
            CGEventType::FlagsChanged => {
                if is_window_picker_active() {
                    return;
                }
                let modifiers_active = flags.contains(self.leader_modifiers);
                LEADER_MODIFIERS_ACTIVE.store(modifiers_active, Ordering::SeqCst);
            }
            CGEventType::KeyDown => {
                if is_window_picker_active() {
                    let has_shift = flags.contains(CGEventFlags::CGEventFlagShift);
                    if let Some(input) = picker_input_from_keycode(keycode, has_shift) {
                        if is_autorepeat {
                            match input {
                                PickerInput::SelectDown
                                | PickerInput::SelectUp
                                | PickerInput::SearchChar('j')
                                | PickerInput::SearchChar('k') => {
                                    let repeat = PICKER_REPEAT_COUNTER.fetch_add(1, Ordering::Relaxed);
                                    if repeat % 2 != 0 {
                                        event.set_type(CGEventType::Null);
                                        return;
                                    }
                                }
                                _ => {
                                    event.set_type(CGEventType::Null);
                                    return;
                                }
                            }
                        } else {
                            PICKER_REPEAT_COUNTER.store(0, Ordering::Relaxed);
                        }
                        tracing::trace!(
                            "picker input from event tap: {:?} (keycode={})",
                            input,
                            keycode
                        );
                        let _ = self.sender.send(EventTapAction::PickerInput(input));
                        event.set_type(CGEventType::Null);
                    }
                    return;
                }
                let mods_active = LEADER_MODIFIERS_ACTIVE.load(Ordering::SeqCst);
                let is_listening = IS_LISTENING.load(Ordering::SeqCst);
                let is_leader_key = keycode == self.leader_keycode;

                // Check if this is the leader key combo (modifiers + leader key pressed together)
                if mods_active && is_leader_key && !is_listening {
                    tracing::trace!("leader combo detected (keycode={})", keycode);
                    IS_LISTENING.store(true, Ordering::SeqCst);
                    let _ = self.sender.send(EventTapAction::LeaderPressed);
                    event.set_type(CGEventType::Null);
                    return;
                }

                // Handle keys while in listening mode (after leader combo released)
                if is_listening {
                    // Check for action keybinds
                    for entry in &self.keybinds {
                        if let Keybind::LeaderPrefixed { code } = &entry.keybind
                            && keycode_to_native(*code) == keycode
                        {
                            tracing::trace!("leader action triggered: {:?}", entry.action);
                            let _ = self
                                .sender
                                .send(EventTapAction::ActionTriggered(entry.action.clone()));
                            IS_LISTENING.store(false, Ordering::SeqCst);
                            event.set_type(CGEventType::Null);
                            return;
                        }
                    }

                    if let Some(direction) = keycode_to_direction(keycode) {
                        tracing::trace!("leader direction triggered: {:?}", direction);
                        let _ = self.sender.send(EventTapAction::ArrowPressed(direction));
                        IS_LISTENING.store(false, Ordering::SeqCst);
                        event.set_type(CGEventType::Null);
                        return;
                    }

                    if let Some(letter) = keycode_to_letter(keycode) {
                        let has_shift = flags.contains(CGEventFlags::CGEventFlagShift);
                        tracing::trace!("leader letter triggered: {} shift={}", letter, has_shift);
                        let _ = self
                            .sender
                            .send(EventTapAction::KeyPressed(keycode, has_shift));
                        IS_LISTENING.store(false, Ordering::SeqCst);
                        event.set_type(CGEventType::Null);
                    }
                }
            }
            _ => {}
        }
    }
}

fn modifiers_to_cg_flags(modifiers: Modifiers) -> CGEventFlags {
    let mut flags = CGEventFlags::empty();

    if modifiers.contains(Modifiers::SUPER) {
        flags.insert(CGEventFlags::CGEventFlagCommand);
    }
    if modifiers.contains(Modifiers::ALT) {
        flags.insert(CGEventFlags::CGEventFlagAlternate);
    }
    if modifiers.contains(Modifiers::SHIFT) {
        flags.insert(CGEventFlags::CGEventFlagShift);
    }
    if modifiers.contains(Modifiers::CONTROL) {
        flags.insert(CGEventFlags::CGEventFlagControl);
    }

    flags
}

fn keycode_to_native(code: KeyCode) -> i64 {
    match code {
        KeyCode::KeyA => 0,
        KeyCode::KeyS => 1,
        KeyCode::KeyD => 2,
        KeyCode::KeyF => 3,
        KeyCode::KeyH => 4,
        KeyCode::KeyG => 5,
        KeyCode::KeyZ => 6,
        KeyCode::KeyX => 7,
        KeyCode::KeyC => 8,
        KeyCode::KeyV => 9,
        KeyCode::KeyB => 11,
        KeyCode::KeyQ => 12,
        KeyCode::KeyW => 13,
        KeyCode::KeyE => 14,
        KeyCode::KeyR => 15,
        KeyCode::KeyY => 16,
        KeyCode::KeyT => 17,
        KeyCode::Digit1 => 18,
        KeyCode::Digit2 => 19,
        KeyCode::Digit3 => 20,
        KeyCode::Digit4 => 21,
        KeyCode::Digit6 => 22,
        KeyCode::Digit5 => 23,
        KeyCode::Equal => 24,
        KeyCode::Digit9 => 25,
        KeyCode::Digit7 => 26,
        KeyCode::Minus => 27,
        KeyCode::Digit8 => 28,
        KeyCode::Digit0 => 29,
        KeyCode::BracketRight => 30,
        KeyCode::KeyO => 31,
        KeyCode::KeyU => 32,
        KeyCode::BracketLeft => 33,
        KeyCode::KeyI => 34,
        KeyCode::KeyP => 35,
        KeyCode::KeyL => 37,
        KeyCode::KeyJ => 38,
        KeyCode::KeyK => 40,
        KeyCode::Quote => 39,
        KeyCode::Semicolon => 41,
        KeyCode::Backslash => 42,
        KeyCode::Comma => 43,
        KeyCode::Slash => 44,
        KeyCode::KeyN => 45,
        KeyCode::KeyM => 46,
        KeyCode::Period => 47,
        KeyCode::Space => 49,
        KeyCode::Escape => 53,
        KeyCode::F1 => 122,
        KeyCode::F2 => 120,
        KeyCode::F3 => 99,
        KeyCode::F4 => 118,
        KeyCode::F5 => 96,
        KeyCode::F6 => 97,
        KeyCode::F7 => 98,
        KeyCode::F8 => 100,
        KeyCode::F9 => 101,
        KeyCode::F10 => 109,
        KeyCode::F11 => 103,
        KeyCode::F12 => 111,
        KeyCode::Enter => 36,
        KeyCode::Tab => 48,
        KeyCode::Backspace => 51,
        KeyCode::Delete => 117,
        KeyCode::Insert => 114,
        KeyCode::Home => 115,
        KeyCode::End => 119,
        KeyCode::PageUp => 116,
        KeyCode::PageDown => 121,
        KeyCode::ArrowLeft => 123,
        KeyCode::ArrowRight => 124,
        KeyCode::ArrowDown => 125,
        KeyCode::ArrowUp => 126,
    }
}

fn keycode_to_letter(keycode: i64) -> Option<char> {
    match keycode {
        0 => Some('a'),
        1 => Some('s'),
        2 => Some('d'),
        3 => Some('f'),
        4 => Some('h'),
        5 => Some('g'),
        6 => Some('z'),
        7 => Some('x'),
        8 => Some('c'),
        9 => Some('v'),
        11 => Some('b'),
        12 => Some('q'),
        13 => Some('w'),
        14 => Some('e'),
        15 => Some('r'),
        16 => Some('y'),
        17 => Some('t'),
        31 => Some('o'),
        32 => Some('u'),
        34 => Some('i'),
        35 => Some('p'),
        38 => Some('j'),
        40 => Some('k'),
        37 => Some('l'),
        46 => Some('m'),
        45 => Some('n'),
        _ => None,
    }
}

fn keycode_to_direction(keycode: i64) -> Option<crate::accessibility::Direction> {
    match keycode {
        123 => Some(crate::accessibility::Direction::Left),
        124 => Some(crate::accessibility::Direction::Right),
        125 => Some(crate::accessibility::Direction::Down),
        126 => Some(crate::accessibility::Direction::Up),
        _ => None,
    }
}

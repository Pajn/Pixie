//! Global hotkey registration and handling
//!
//! This module manages the global shortcuts for the leader key and letter keys.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use global_hotkey::{
    hotkey::{Code, HotKey, Modifiers},
    GlobalHotKeyManager,
};

use crate::accessibility::Direction;
use crate::config::{Action, Keybind, KeybindEntry};
use crate::error::Result;

#[derive(Debug, Clone)]
pub struct HotkeyConfig {
    pub leader: (Option<Modifiers>, Code),
    pub keybinds: Vec<KeybindEntry>,
}

impl Default for HotkeyConfig {
    fn default() -> Self {
        HotkeyConfig {
            leader: (Some(Modifiers::SUPER | Modifiers::SHIFT), Code::KeyA),
            keybinds: Vec::new(),
        }
    }
}

pub struct HotkeyManager {
    manager: GlobalHotKeyManager,
    config: HotkeyConfig,
    pub leader_id: u32,
    letter_definitions: Vec<(Code, char)>,
    letter_hotkeys: HashMap<u32, (char, bool)>,
    arrow_hotkeys: HashMap<u32, Direction>,
    registered_hotkeys: Mutex<Vec<HotKey>>,
    letters_registered: AtomicBool,
    direct_keybinds: HashMap<u32, Action>,
    leader_keybinds: Mutex<HashMap<u32, Action>>,
    leader_keybind_definitions: Vec<(Code, Action)>,
}

impl HotkeyManager {
    pub fn new() -> Result<Self> {
        Self::with_config(HotkeyConfig::default())
    }

    pub fn with_config(config: HotkeyConfig) -> Result<Self> {
        let manager = GlobalHotKeyManager::new()?;

        let leader_hotkey = HotKey::new(config.leader.0, config.leader.1);
        let leader_id = leader_hotkey.id();
        manager.register(leader_hotkey)?;

        tracing::info!(
            "Registered hotkey: Leader={:?}+{:?} (id={})",
            config.leader.0,
            config.leader.1,
            leader_id
        );

        let letter_codes = [
            Code::KeyA,
            Code::KeyB,
            Code::KeyC,
            Code::KeyD,
            Code::KeyE,
            Code::KeyF,
            Code::KeyG,
            Code::KeyH,
            Code::KeyI,
            Code::KeyJ,
            Code::KeyK,
            Code::KeyL,
            Code::KeyM,
            Code::KeyN,
            Code::KeyO,
            Code::KeyP,
            Code::KeyQ,
            Code::KeyR,
            Code::KeyS,
            Code::KeyT,
            Code::KeyU,
            Code::KeyV,
            Code::KeyW,
            Code::KeyX,
            Code::KeyY,
            Code::KeyZ,
        ];

        let letters = "abcdefghijklmnopqrstuvwxyz".chars().collect::<Vec<_>>();
        let letter_definitions: Vec<(Code, char)> = letter_codes
            .iter()
            .zip(letters.iter())
            .map(|(code, letter)| (*code, *letter))
            .collect();

        let mut letter_hotkeys = HashMap::new();
        for (code, letter) in &letter_definitions {
            let hotkey = HotKey::new(None, *code);
            letter_hotkeys.insert(hotkey.id(), (*letter, false));

            let shift_hotkey = HotKey::new(Some(Modifiers::SHIFT), *code);
            letter_hotkeys.insert(shift_hotkey.id(), (*letter, true));
        }

        let arrow_codes = [
            (Code::ArrowLeft, Direction::Left),
            (Code::ArrowRight, Direction::Right),
            (Code::ArrowUp, Direction::Up),
            (Code::ArrowDown, Direction::Down),
        ];

        let mut arrow_hotkeys = HashMap::new();
        for (code, direction) in arrow_codes {
            let hotkey = HotKey::new(None, code);
            arrow_hotkeys.insert(hotkey.id(), direction);
        }

        tracing::info!(
            "Prepared {} letter hotkey definitions (not registered yet)",
            letter_definitions.len()
        );

        let mut direct_keybinds = HashMap::new();
        let mut leader_keybind_definitions = Vec::new();

        for entry in &config.keybinds {
            match &entry.keybind {
                Keybind::Direct { modifiers, code } => {
                    let hotkey = HotKey::new(*modifiers, *code);
                    let id = hotkey.id();
                    if let Err(e) = manager.register(hotkey) {
                        tracing::warn!(
                            "Failed to register direct keybind {:?}: {}",
                            entry.keybind,
                            e
                        );
                    } else {
                        direct_keybinds.insert(id, entry.action);
                        tracing::info!(
                            "Registered direct keybind: {:?} -> {:?} (id={})",
                            entry.keybind,
                            entry.action,
                            id
                        );
                    }
                }
                Keybind::LeaderPrefixed { code } => {
                    leader_keybind_definitions.push((*code, entry.action));
                }
            }
        }

        Ok(HotkeyManager {
            manager,
            config,
            leader_id,
            letter_definitions,
            letter_hotkeys,
            arrow_hotkeys,
            registered_hotkeys: Mutex::new(Vec::new()),
            letters_registered: AtomicBool::new(false),
            direct_keybinds,
            leader_keybinds: Mutex::new(HashMap::new()),
            leader_keybind_definitions,
        })
    }

    pub fn register_letter_hotkeys(&self) -> Result<()> {
        if self.letters_registered.load(Ordering::SeqCst) {
            return Ok(());
        }

        let mut registered = self.registered_hotkeys.lock().unwrap();

        for (code, _) in &self.letter_definitions {
            let hotkey = HotKey::new(None, *code);
            if let Err(e) = self.manager.register(hotkey) {
                tracing::warn!("Failed to register letter hotkey {:?}: {}", code, e);
            } else {
                registered.push(hotkey);
            }

            let shift_hotkey = HotKey::new(Some(Modifiers::SHIFT), *code);
            if let Err(e) = self.manager.register(shift_hotkey) {
                tracing::warn!("Failed to register shift+letter hotkey {:?}: {}", code, e);
            } else {
                registered.push(shift_hotkey);
            }
        }

        for (code, action) in &self.leader_keybind_definitions {
            let hotkey = HotKey::new(None, *code);
            let id = hotkey.id();
            if let Err(e) = self.manager.register(hotkey) {
                tracing::warn!(
                    "Failed to register leader-prefixed keybind {:?}: {}",
                    code,
                    e
                );
            } else {
                registered.push(hotkey);
                self.leader_keybinds.lock().unwrap().insert(id, *action);
                tracing::info!(
                    "Registered leader-prefixed keybind: {:?} -> {:?} (id={})",
                    code,
                    action,
                    id
                );
            }
        }

        for (code, direction) in [
            (Code::ArrowLeft, Direction::Left),
            (Code::ArrowRight, Direction::Right),
            (Code::ArrowUp, Direction::Up),
            (Code::ArrowDown, Direction::Down),
        ] {
            let hotkey = HotKey::new(None, code);
            if let Err(e) = self.manager.register(hotkey) {
                tracing::warn!("Failed to register arrow hotkey {:?}: {}", code, e);
            } else {
                registered.push(hotkey);
                tracing::info!(
                    "Registered arrow hotkey: {:?} -> {:?} (id={})",
                    code,
                    direction,
                    hotkey.id()
                );
            }
        }

        self.letters_registered.store(true, Ordering::SeqCst);
        tracing::info!("Registered {} letter hotkeys", registered.len());

        Ok(())
    }

    pub fn unregister_letter_hotkeys(&self) -> Result<()> {
        if !self.letters_registered.load(Ordering::SeqCst) {
            return Ok(());
        }

        let mut registered = self.registered_hotkeys.lock().unwrap();

        for hotkey in registered.iter() {
            if let Err(e) = self.manager.unregister(*hotkey) {
                tracing::warn!("Failed to unregister hotkey: {}", e);
            }
        }

        registered.clear();
        self.leader_keybinds.lock().unwrap().clear();
        self.letters_registered.store(false, Ordering::SeqCst);
        tracing::info!("Unregistered all letter hotkeys");

        Ok(())
    }

    pub fn is_letters_registered(&self) -> bool {
        self.letters_registered.load(Ordering::SeqCst)
    }

    pub fn get_letter_info(&self, id: u32) -> Option<(char, bool)> {
        self.letter_hotkeys.get(&id).copied()
    }

    pub fn get_direct_keybind_action(&self, id: u32) -> Option<Action> {
        self.direct_keybinds.get(&id).copied()
    }

    pub fn get_leader_keybind_action(&self, id: u32) -> Option<Action> {
        self.leader_keybinds.lock().unwrap().get(&id).copied()
    }

    pub fn get_arrow_direction(&self, id: u32) -> Option<Direction> {
        self.arrow_hotkeys.get(&id).copied()
    }

    pub fn unregister(&self) -> Result<()> {
        let leader_hotkey = HotKey::new(self.config.leader.0, self.config.leader.1);
        self.manager.unregister(leader_hotkey)?;
        self.unregister_letter_hotkeys()?;

        tracing::info!("Unregistered all hotkeys");

        Ok(())
    }
}

impl Default for HotkeyManager {
    fn default() -> Self {
        Self::new().expect("Failed to create HotkeyManager")
    }
}

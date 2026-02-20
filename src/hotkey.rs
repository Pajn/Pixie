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

use crate::error::Result;

#[derive(Debug, Clone)]
pub struct HotkeyConfig {
    pub leader: (Option<Modifiers>, Code),
}

impl Default for HotkeyConfig {
    fn default() -> Self {
        HotkeyConfig {
            leader: (Some(Modifiers::SUPER | Modifiers::SHIFT), Code::KeyA),
        }
    }
}

pub struct HotkeyManager {
    manager: GlobalHotKeyManager,
    config: HotkeyConfig,
    pub leader_id: u32,
    letter_definitions: Vec<(Code, char)>,
    letter_hotkeys: HashMap<u32, (char, bool)>,
    registered_hotkeys: Mutex<Vec<HotKey>>,
    letters_registered: AtomicBool,
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

        tracing::info!(
            "Prepared {} letter hotkey definitions (not registered yet)",
            letter_definitions.len()
        );

        Ok(HotkeyManager {
            manager,
            config,
            leader_id,
            letter_definitions,
            letter_hotkeys,
            registered_hotkeys: Mutex::new(Vec::new()),
            letters_registered: AtomicBool::new(false),
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

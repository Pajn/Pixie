//! Global hotkey registration and handling
//!
//! This module manages the global shortcuts for the leader key and letter keys.

use std::collections::HashMap;

use global_hotkey::{
    hotkey::{Code, HotKey, Modifiers},
    GlobalHotKeyManager,
};

use crate::error::Result;

/// Configuration for hotkeys
#[derive(Debug, Clone)]
pub struct HotkeyConfig {
    /// Hotkey for the leader key (default: Cmd+Shift+A)
    pub leader: (Option<Modifiers>, Code),
}

impl Default for HotkeyConfig {
    fn default() -> Self {
        HotkeyConfig {
            leader: (Some(Modifiers::SUPER | Modifiers::SHIFT), Code::KeyA),
        }
    }
}

/// Hotkey manager that handles registration and events
pub struct HotkeyManager {
    #[allow(dead_code)]
    manager: GlobalHotKeyManager,
    config: HotkeyConfig,
    /// ID of the leader hotkey
    pub leader_id: u32,
    /// Mapping from hotkey ID to (letter, has_shift)
    pub letter_hotkeys: HashMap<u32, (char, bool)>,
}

impl HotkeyManager {
    /// Create a new hotkey manager with default configuration
    pub fn new() -> Result<Self> {
        Self::with_config(HotkeyConfig::default())
    }

    /// Create a new hotkey manager with custom configuration
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

        let mut letter_hotkeys = HashMap::new();

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

        for (code, letter) in letter_codes.iter().zip(letters.iter()) {
            let hotkey = HotKey::new(None, *code);
            let id = hotkey.id();
            manager.register(hotkey)?;
            letter_hotkeys.insert(id, (*letter, false));

            let shift_hotkey = HotKey::new(Some(Modifiers::SHIFT), *code);
            let shift_id = shift_hotkey.id();
            manager.register(shift_hotkey)?;
            letter_hotkeys.insert(shift_id, (*letter, true));
        }

        tracing::info!(
            "Registered {} letter hotkeys (a-z and Shift+A-Z)",
            letter_hotkeys.len()
        );

        Ok(HotkeyManager {
            manager,
            config,
            leader_id,
            letter_hotkeys,
        })
    }

    /// Unregister the hotkey
    pub fn unregister(&self) -> Result<()> {
        let leader_hotkey = HotKey::new(self.config.leader.0, self.config.leader.1);
        self.manager.unregister(leader_hotkey)?;

        tracing::info!("Unregistered hotkey");

        Ok(())
    }
}

impl Default for HotkeyManager {
    fn default() -> Self {
        Self::new().expect("Failed to create HotkeyManager")
    }
}

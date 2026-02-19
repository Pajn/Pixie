//! Global hotkey registration and handling
//!
//! This module manages the global shortcuts for registering and focusing windows.

use global_hotkey::{
    hotkey::{Code, HotKey, Modifiers},
    GlobalHotKeyManager,
};

use crate::error::Result;

/// Configuration for hotkeys
#[derive(Debug, Clone)]
pub struct HotkeyConfig {
    /// Hotkey for registering a window (default: Cmd+Shift+R)
    pub register: (Option<Modifiers>, Code),
    /// Hotkey for focusing the saved window (default: Cmd+Shift+F)
    pub focus: (Option<Modifiers>, Code),
}

impl Default for HotkeyConfig {
    fn default() -> Self {
        HotkeyConfig {
            register: (Some(Modifiers::SUPER | Modifiers::SHIFT), Code::KeyR),
            focus: (Some(Modifiers::SUPER | Modifiers::SHIFT), Code::KeyA),
        }
    }
}

/// Hotkey manager that handles registration and events
pub struct HotkeyManager {
    #[allow(dead_code)]
    manager: GlobalHotKeyManager,
    config: HotkeyConfig,
    /// ID of the register hotkey
    pub register_id: u32,
    /// ID of the focus hotkey
    pub focus_id: u32,
}

impl HotkeyManager {
    /// Create a new hotkey manager with default configuration
    pub fn new() -> Result<Self> {
        Self::with_config(HotkeyConfig::default())
    }

    /// Create a new hotkey manager with custom configuration
    pub fn with_config(config: HotkeyConfig) -> Result<Self> {
        let manager = GlobalHotKeyManager::new()?;

        // Register the register hotkey and get its ID
        let register_hotkey = HotKey::new(config.register.0, config.register.1);
        let register_id = register_hotkey.id();
        manager.register(register_hotkey)?;

        // Register the focus hotkey and get its ID
        let focus_hotkey = HotKey::new(config.focus.0, config.focus.1);
        let focus_id = focus_hotkey.id();
        manager.register(focus_hotkey)?;

        tracing::info!(
            "Registered hotkeys: Register={:?}+{:?} (id={}), Focus={:?}+{:?} (id={})",
            config.register.0,
            config.register.1,
            register_id,
            config.focus.0,
            config.focus.1,
            focus_id
        );

        Ok(HotkeyManager {
            manager,
            config,
            register_id,
            focus_id,
        })
    }

    /// Unregister all hotkeys
    pub fn unregister_all(&self) -> Result<()> {
        let register_hotkey = HotKey::new(self.config.register.0, self.config.register.1);
        let focus_hotkey = HotKey::new(self.config.focus.0, self.config.focus.1);

        self.manager.unregister(register_hotkey)?;
        self.manager.unregister(focus_hotkey)?;

        tracing::info!("Unregistered all hotkeys");

        Ok(())
    }
}

impl Default for HotkeyManager {
    fn default() -> Self {
        Self::new().expect("Failed to create HotkeyManager")
    }
}

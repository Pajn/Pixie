//! Window management - capture and focus operations
//!
//! This module handles saving and recalling window state.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::accessibility;
use crate::error::PixieError;

/// Saved window state that can be persisted
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedWindow {
    /// Process ID of the application
    pub pid: i32,
    /// CGWindowID for finding the specific window
    pub window_id: u32,
    /// Application name for display
    pub app_name: String,
    /// Window title for display
    pub title: String,
}

/// Window manager that handles saving and focusing windows
pub struct WindowManager {
    /// Saved windows indexed by single character keys
    saved_windows: Arc<Mutex<HashMap<char, SavedWindow>>>,
    /// Path to the persistence file
    config_path: std::path::PathBuf,
}

impl WindowManager {
    /// Create a new window manager
    pub fn new() -> Result<Self, PixieError> {
        let config_dir = dirs::config_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("pixie");

        // Ensure config directory exists
        std::fs::create_dir_all(&config_dir)
            .map_err(|e| PixieError::Config(format!("Failed to create config directory: {}", e)))?;

        let config_path = config_dir.join("saved_windows.json");

        let manager = WindowManager {
            saved_windows: Arc::new(Mutex::new(HashMap::new())),
            config_path,
        };

        // Load saved windows from disk
        manager.load_saved_windows()?;

        Ok(manager)
    }

    /// Get a saved window by key
    pub fn get_saved_window(&self, key: char) -> Option<SavedWindow> {
        self.saved_windows.lock().unwrap().get(&key).cloned()
    }

    /// Get all saved windows
    pub fn get_all_saved_windows(&self) -> HashMap<char, SavedWindow> {
        self.saved_windows.lock().unwrap().clone()
    }

    /// Register (save) the currently focused window to a slot
    pub fn register_current_window(&self, key: char) -> Result<(char, SavedWindow), PixieError> {
        // Get the focused window element (retry to handle hotkey-timing race)
        let element = accessibility::get_focused_window_with_retry(10, Duration::from_millis(50))?;

        // Get window info
        let info = accessibility::get_window_info(&element)?;

        // Get the window ID
        let window_id = accessibility::get_window_id(&element)?;

        // Get application name
        let app_name = accessibility::get_app_name(info.pid)?;

        // Create saved window
        let saved = SavedWindow {
            pid: info.pid,
            window_id,
            app_name: app_name.clone(),
            title: info.title.clone(),
        };

        // Save to memory and disk
        {
            let mut guard = self.saved_windows.lock().unwrap();
            guard.insert(key, saved.clone());
        }
        self.save_to_disk()?;

        tracing::info!(
            "Registered window to slot '{}': {} - {:?}",
            key,
            app_name,
            info.title
        );

        Ok((key, saved))
    }

    /// Focus the saved window at the given slot
    pub fn focus_saved_window(&self, key: char) -> Result<SavedWindow, PixieError> {
        let saved = self.saved_windows.lock().unwrap().get(&key).cloned();

        let saved = saved.ok_or_else(|| {
            PixieError::Config(format!("No window registered for slot '{}'", key))
        })?;

        // Find the window element by PID and window ID
        let element = accessibility::find_window_by_id(saved.pid, saved.window_id)?;

        // Focus the window
        accessibility::focus_window(&element)?;

        tracing::info!(
            "Focused window at slot '{}': {} - {:?}",
            key,
            saved.app_name,
            saved.title
        );

        Ok(saved)
    }

    /// Clear a specific slot, returns true if a window was removed
    pub fn clear_slot(&self, key: char) -> Result<bool, PixieError> {
        let existed = {
            let mut guard = self.saved_windows.lock().unwrap();
            guard.remove(&key).is_some()
        };

        if existed {
            self.save_to_disk()?;
            tracing::info!("Cleared window at slot '{}'", key);
        }

        Ok(existed)
    }

    /// Clear all saved windows
    pub fn clear_all_windows(&self) -> Result<(), PixieError> {
        {
            let mut guard = self.saved_windows.lock().unwrap();
            guard.clear();
        }

        // Remove the config file
        if self.config_path.exists() {
            std::fs::remove_file(&self.config_path)
                .map_err(|e| PixieError::Config(format!("Failed to remove config: {}", e)))?;
        }

        tracing::info!("Cleared all saved windows");

        Ok(())
    }

    /// Save all window states to disk
    fn save_to_disk(&self) -> Result<(), PixieError> {
        let guard = self.saved_windows.lock().unwrap();
        let json = serde_json::to_string_pretty(&*guard)
            .map_err(|e| PixieError::Config(format!("Failed to serialize windows: {}", e)))?;

        std::fs::write(&self.config_path, json)
            .map_err(|e| PixieError::Config(format!("Failed to write config: {}", e)))?;

        Ok(())
    }

    /// Load the saved windows from disk
    fn load_saved_windows(&self) -> Result<(), PixieError> {
        if !self.config_path.exists() {
            return Ok(());
        }

        let json = std::fs::read_to_string(&self.config_path)
            .map_err(|e| PixieError::Config(format!("Failed to read config: {}", e)))?;

        let saved: HashMap<char, SavedWindow> = serde_json::from_str(&json)
            .map_err(|e| PixieError::Config(format!("Failed to parse config: {}", e)))?;

        {
            let mut guard = self.saved_windows.lock().unwrap();
            *guard = saved;
        }

        Ok(())
    }
}

impl Default for WindowManager {
    fn default() -> Self {
        Self::new().expect("Failed to create WindowManager")
    }
}

impl SavedWindow {
    /// Get a display string for the window
    pub fn display_string(&self) -> String {
        if self.title.is_empty() {
            format!("{} (PID: {})", self.app_name, self.pid)
        } else {
            format!("{} - \"{}\"", self.app_name, self.title)
        }
    }
}

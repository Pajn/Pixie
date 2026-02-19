//! Window management - capture and focus operations
//!
//! This module handles saving and recalling window state.

use serde::{Deserialize, Serialize};
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
    /// The currently saved window
    saved_window: Arc<Mutex<Option<SavedWindow>>>,
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

        let config_path = config_dir.join("saved_window.json");

        let manager = WindowManager {
            saved_window: Arc::new(Mutex::new(None)),
            config_path,
        };

        // Load saved window from disk
        manager.load_saved_window()?;

        Ok(manager)
    }

    /// Get the currently saved window
    pub fn get_saved_window(&self) -> Option<SavedWindow> {
        self.saved_window.lock().unwrap().clone()
    }

    /// Register (save) the currently focused window
    pub fn register_current_window(&self) -> Result<SavedWindow, PixieError> {
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
            let mut guard = self.saved_window.lock().unwrap();
            *guard = Some(saved.clone());
        }
        self.save_to_disk(&saved)?;

        tracing::info!("Registered window: {} - {:?}", app_name, info.title);

        Ok(saved)
    }

    /// Focus the saved window
    pub fn focus_saved_window(&self) -> Result<SavedWindow, PixieError> {
        let saved = self.saved_window.lock().unwrap().clone();

        let saved = saved.ok_or(PixieError::NoWindowRegistered)?;

        // Find the window element by PID and window ID
        let element = accessibility::find_window_by_id(saved.pid, saved.window_id)?;

        // Focus the window
        accessibility::focus_window(&element)?;

        tracing::info!("Focused window: {} - {:?}", saved.app_name, saved.title);

        Ok(saved)
    }

    /// Save the window state to disk
    fn save_to_disk(&self, window: &SavedWindow) -> Result<(), PixieError> {
        let json = serde_json::to_string_pretty(window)
            .map_err(|e| PixieError::Config(format!("Failed to serialize window: {}", e)))?;

        std::fs::write(&self.config_path, json)
            .map_err(|e| PixieError::Config(format!("Failed to write config: {}", e)))?;

        Ok(())
    }

    /// Load the saved window from disk
    fn load_saved_window(&self) -> Result<(), PixieError> {
        if !self.config_path.exists() {
            return Ok(());
        }

        let json = std::fs::read_to_string(&self.config_path)
            .map_err(|e| PixieError::Config(format!("Failed to read config: {}", e)))?;

        let saved: SavedWindow = serde_json::from_str(&json)
            .map_err(|e| PixieError::Config(format!("Failed to parse config: {}", e)))?;

        {
            let mut guard = self.saved_window.lock().unwrap();
            *guard = Some(saved);
        }

        Ok(())
    }

    /// Clear the saved window
    pub fn clear_saved_window(&self) -> Result<(), PixieError> {
        {
            let mut guard = self.saved_window.lock().unwrap();
            *guard = None;
        }

        // Remove the config file
        if self.config_path.exists() {
            std::fs::remove_file(&self.config_path)
                .map_err(|e| PixieError::Config(format!("Failed to remove config: {}", e)))?;
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

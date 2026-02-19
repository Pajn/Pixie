//! Menu bar UI for Pixie

use std::sync::Arc;

use crate::error::Result;
use crate::window::WindowManager;

/// Menu bar controller
pub struct MenuBarController {
    #[allow(dead_code)]
    window_manager: Arc<WindowManager>,
}

impl MenuBarController {
    /// Create a new menu bar controller
    pub fn new(window_manager: Arc<WindowManager>) -> Result<Self> {
        // We need to run this on the main thread
        // The menu bar will be created when the app starts
        Ok(MenuBarController { window_manager })
    }
}

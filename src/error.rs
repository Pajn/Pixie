//! Error types for Pixie

use thiserror::Error;

#[allow(dead_code)]
#[derive(Error, Debug)]
pub enum PixieError {
    #[error("Accessibility API error: {0}")]
    Accessibility(String),

    #[error("Hotkey error: {0}")]
    Hotkey(String),

    #[error("No window registered")]
    NoWindowRegistered,

    #[error("Window not found")]
    WindowNotFound,

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Menu bar error: {0}")]
    MenuBar(String),

    #[error("Leader mode error: {0}")]
    LeaderMode(String),

    #[error("Event tap error: {0}")]
    EventTap(String),
}

pub type Result<T> = std::result::Result<T, PixieError>;

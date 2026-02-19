//! Error types for Pixie

use thiserror::Error;

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
}

impl From<global_hotkey::Error> for PixieError {
    fn from(e: global_hotkey::Error) -> Self {
        PixieError::Hotkey(e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, PixieError>;

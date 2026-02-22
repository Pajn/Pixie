//! Configuration management for Pixie
//!
//! Handles TOML config file parsing and LaunchAgent management for autostart.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

use serde::{Deserialize, Serialize};

use crate::error::{PixieError, Result};

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct Modifiers: u32 {
        const SUPER = 1 << 0;
        const ALT = 1 << 1;
        const SHIFT = 1 << 2;
        const CONTROL = 1 << 3;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyCode {
    KeyA,
    KeyB,
    KeyC,
    KeyD,
    KeyE,
    KeyF,
    KeyG,
    KeyH,
    KeyI,
    KeyJ,
    KeyK,
    KeyL,
    KeyM,
    KeyN,
    KeyO,
    KeyP,
    KeyQ,
    KeyR,
    KeyS,
    KeyT,
    KeyU,
    KeyV,
    KeyW,
    KeyX,
    KeyY,
    KeyZ,
    Digit0,
    Digit1,
    Digit2,
    Digit3,
    Digit4,
    Digit5,
    Digit6,
    Digit7,
    Digit8,
    Digit9,
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
    Space,
    Escape,
    Enter,
    Tab,
    Backspace,
    Delete,
    Insert,
    Home,
    End,
    PageUp,
    PageDown,
    ArrowLeft,
    ArrowRight,
    ArrowUp,
    ArrowDown,
    Equal,
    Minus,
    BracketLeft,
    BracketRight,
    Backslash,
    Semicolon,
    Quote,
    Comma,
    Period,
    Slash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    FocusLeft,
    FocusRight,
    FocusUp,
    FocusDown,
    Minimize,
    Maximize,
    Fullscreen,
    Center,
    MoveMonitorLeft,
    MoveMonitorRight,
    MoveMonitorUp,
    MoveMonitorDown,
    Place(String),
    #[serde(rename = "tile")]
    Tile,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Placement {
    #[serde(default)]
    pub top: Option<String>,
    #[serde(default)]
    pub left: Option<String>,
    #[serde(default)]
    pub width: Option<String>,
    #[serde(default)]
    pub height: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Keybind {
    Direct {
        modifiers: Option<Modifiers>,
        code: KeyCode,
    },
    LeaderPrefixed {
        code: KeyCode,
    },
}

#[derive(Debug, Clone)]
pub struct KeybindEntry {
    pub keybind: Keybind,
    pub action: Action,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_leader_key")]
    pub leader_key: String,

    #[serde(default)]
    pub autostart: bool,

    #[serde(default = "default_timeout")]
    pub timeout: u64,

    #[serde(default)]
    pub keybinds: HashMap<String, Action>,

    #[serde(default)]
    pub placements: HashMap<String, Placement>,

    #[serde(default = "default_menubar_icon")]
    pub menubar_icon: bool,

    #[serde(default)]
    pub menubar_active_color: Option<String>,
}

fn default_leader_key() -> String {
    "cmd+shift+a".to_string()
}

fn default_timeout() -> u64 {
    2
}

fn default_menubar_icon() -> bool {
    true
}

impl Default for Config {
    fn default() -> Self {
        Config {
            leader_key: default_leader_key(),
            autostart: false,
            timeout: default_timeout(),
            keybinds: HashMap::new(),
            placements: HashMap::new(),
            menubar_icon: default_menubar_icon(),
            menubar_active_color: None,
        }
    }
}

impl Config {
    pub fn parse_keybind(key: &str) -> Result<Keybind> {
        let key_lower = key.to_lowercase();

        if let Some(rest) = key_lower.strip_prefix("leader+") {
            let code = parse_key_code(rest.trim())?;
            Ok(Keybind::LeaderPrefixed { code })
        } else {
            let (modifiers, code) = parse_leader_key(key)?;
            Ok(Keybind::Direct { modifiers, code })
        }
    }

    pub fn parsed_keybinds(&self) -> Vec<KeybindEntry> {
        self.keybinds
            .iter()
            .filter_map(|(key, action)| {
                Self::parse_keybind(key).ok().map(|keybind| KeybindEntry {
                    keybind,
                    action: action.clone(),
                })
            })
            .collect()
    }

    pub fn get_placements(&self) -> HashMap<String, Placement> {
        let mut placements = builtin_placements();
        placements.extend(self.placements.clone());
        placements
    }
}

fn config_path() -> PathBuf {
    let mut path = dirs::config_local_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("pixie");
    path.push("config.toml");
    path
}

pub fn load() -> Result<Config> {
    let path = config_path();

    match fs::read_to_string(&path) {
        Ok(content) => toml::from_str(&content).map_err(|e| {
            PixieError::Config(format!(
                "Failed to parse config file at {:?}:\n  {}",
                path, e
            ))
        }),
        Err(_) => Ok(Config::default()),
    }
}

pub fn builtin_placements() -> HashMap<String, Placement> {
    let mut placements = HashMap::new();

    placements.insert(
        "left".to_string(),
        Placement {
            left: Some("0%".to_string()),
            width: Some("50%".to_string()),
            height: Some("100%".to_string()),
            top: None,
        },
    );

    placements.insert(
        "right".to_string(),
        Placement {
            left: Some("50%".to_string()),
            width: Some("50%".to_string()),
            height: Some("100%".to_string()),
            top: None,
        },
    );

    placements.insert(
        "top".to_string(),
        Placement {
            top: Some("0%".to_string()),
            width: Some("100%".to_string()),
            height: Some("50%".to_string()),
            left: None,
        },
    );

    placements.insert(
        "bottom".to_string(),
        Placement {
            top: Some("50%".to_string()),
            width: Some("100%".to_string()),
            height: Some("50%".to_string()),
            left: None,
        },
    );

    placements.insert(
        "top_left".to_string(),
        Placement {
            top: Some("0%".to_string()),
            left: Some("0%".to_string()),
            width: Some("50%".to_string()),
            height: Some("50%".to_string()),
        },
    );

    placements.insert(
        "top_right".to_string(),
        Placement {
            top: Some("0%".to_string()),
            left: Some("50%".to_string()),
            width: Some("50%".to_string()),
            height: Some("50%".to_string()),
        },
    );

    placements.insert(
        "bottom_left".to_string(),
        Placement {
            top: Some("50%".to_string()),
            left: Some("0%".to_string()),
            width: Some("50%".to_string()),
            height: Some("50%".to_string()),
        },
    );

    placements.insert(
        "bottom_right".to_string(),
        Placement {
            top: Some("50%".to_string()),
            left: Some("50%".to_string()),
            width: Some("50%".to_string()),
            height: Some("50%".to_string()),
        },
    );

    placements.insert(
        "center".to_string(),
        Placement {
            top: Some("center".to_string()),
            left: Some("center".to_string()),
            width: None,
            height: None,
        },
    );

    placements
}

pub fn parse_percentage(s: &str) -> Result<f64> {
    let s = s.trim();
    if !s.ends_with('%') {
        return Err(PixieError::Config(format!(
            "Invalid percentage format: {}",
            s
        )));
    }

    let num_str = &s[..s.len() - 1];
    let num: f64 = num_str
        .parse()
        .map_err(|e| PixieError::Config(format!("Invalid percentage number: {}", e)))?;

    Ok(num / 100.0)
}

pub fn parse_position_value(s: &str, screen_size: f64, window_size: f64) -> Result<f64> {
    let s = s.trim();
    if s == "center" {
        return Ok((screen_size - window_size) / 2.0);
    }
    let pct = parse_percentage(s)?;
    Ok(pct * screen_size)
}

pub fn parse_size_value(s: &str, screen_size: f64) -> Result<f64> {
    let pct = parse_percentage(s)?;
    Ok(pct * screen_size)
}

pub fn parse_leader_key(key: &str) -> Result<(Option<Modifiers>, KeyCode)> {
    let key_lower = key.to_lowercase();
    let parts: Vec<&str> = key_lower.split('+').collect();

    if parts.is_empty() {
        return Err(PixieError::Config("Empty leader key".to_string()));
    }

    let mut modifiers = None;
    let mut code = None;

    for (i, part) in parts.iter().enumerate() {
        let part = part.trim();

        if i == parts.len() - 1 {
            code = Some(parse_key_code(part)?);
        } else {
            let modifier = parse_modifier(part)?;
            modifiers = Some(match modifiers {
                Some(m) => m | modifier,
                None => modifier,
            });
        }
    }

    let code =
        code.ok_or_else(|| PixieError::Config("No key specified in leader key".to_string()))?;

    Ok((modifiers, code))
}

fn parse_modifier(s: &str) -> Result<Modifiers> {
    match s {
        "cmd" | "super" => Ok(Modifiers::SUPER),
        "alt" | "option" => Ok(Modifiers::ALT),
        "shift" => Ok(Modifiers::SHIFT),
        "ctrl" | "control" => Ok(Modifiers::CONTROL),
        _ => Err(PixieError::Config(format!("Unknown modifier: {}", s))),
    }
}

fn special_key_to_code(s: &str) -> Option<KeyCode> {
    match s.to_lowercase().as_str() {
        "space" => Some(KeyCode::Space),
        "escape" | "esc" => Some(KeyCode::Escape),
        "enter" | "return" => Some(KeyCode::Enter),
        "tab" => Some(KeyCode::Tab),
        "backspace" => Some(KeyCode::Backspace),
        "delete" => Some(KeyCode::Delete),
        "insert" => Some(KeyCode::Insert),
        "home" => Some(KeyCode::Home),
        "end" => Some(KeyCode::End),
        "pageup" => Some(KeyCode::PageUp),
        "pagedown" => Some(KeyCode::PageDown),
        "up" => Some(KeyCode::ArrowUp),
        "down" => Some(KeyCode::ArrowDown),
        "left" => Some(KeyCode::ArrowLeft),
        "right" => Some(KeyCode::ArrowRight),
        _ => None,
    }
}

fn parse_key_code(s: &str) -> Result<KeyCode> {
    if let Some(code) = special_key_to_code(s) {
        return Ok(code);
    }

    if s.len() == 1 {
        let c = s.chars().next().unwrap();
        if c.is_ascii_lowercase() {
            return char_to_code(c);
        }
        if c.is_ascii_digit() {
            return digit_to_code(c);
        }
    }

    if s.starts_with('f') || s.starts_with('F') {
        return function_key_to_code(s);
    }

    Err(PixieError::Config(format!("Unknown key: {}", s)))
}

fn char_to_code(c: char) -> Result<KeyCode> {
    match c {
        'a' => Ok(KeyCode::KeyA),
        'b' => Ok(KeyCode::KeyB),
        'c' => Ok(KeyCode::KeyC),
        'd' => Ok(KeyCode::KeyD),
        'e' => Ok(KeyCode::KeyE),
        'f' => Ok(KeyCode::KeyF),
        'g' => Ok(KeyCode::KeyG),
        'h' => Ok(KeyCode::KeyH),
        'i' => Ok(KeyCode::KeyI),
        'j' => Ok(KeyCode::KeyJ),
        'k' => Ok(KeyCode::KeyK),
        'l' => Ok(KeyCode::KeyL),
        'm' => Ok(KeyCode::KeyM),
        'n' => Ok(KeyCode::KeyN),
        'o' => Ok(KeyCode::KeyO),
        'p' => Ok(KeyCode::KeyP),
        'q' => Ok(KeyCode::KeyQ),
        'r' => Ok(KeyCode::KeyR),
        's' => Ok(KeyCode::KeyS),
        't' => Ok(KeyCode::KeyT),
        'u' => Ok(KeyCode::KeyU),
        'v' => Ok(KeyCode::KeyV),
        'w' => Ok(KeyCode::KeyW),
        'x' => Ok(KeyCode::KeyX),
        'y' => Ok(KeyCode::KeyY),
        'z' => Ok(KeyCode::KeyZ),
        _ => Err(PixieError::Config(format!("Invalid letter key: {}", c))),
    }
}

fn digit_to_code(c: char) -> Result<KeyCode> {
    match c {
        '0' => Ok(KeyCode::Digit0),
        '1' => Ok(KeyCode::Digit1),
        '2' => Ok(KeyCode::Digit2),
        '3' => Ok(KeyCode::Digit3),
        '4' => Ok(KeyCode::Digit4),
        '5' => Ok(KeyCode::Digit5),
        '6' => Ok(KeyCode::Digit6),
        '7' => Ok(KeyCode::Digit7),
        '8' => Ok(KeyCode::Digit8),
        '9' => Ok(KeyCode::Digit9),
        _ => Err(PixieError::Config(format!("Invalid digit key: {}", c))),
    }
}

fn function_key_to_code(s: &str) -> Result<KeyCode> {
    match s.to_uppercase().as_str() {
        "F1" => Ok(KeyCode::F1),
        "F2" => Ok(KeyCode::F2),
        "F3" => Ok(KeyCode::F3),
        "F4" => Ok(KeyCode::F4),
        "F5" => Ok(KeyCode::F5),
        "F6" => Ok(KeyCode::F6),
        "F7" => Ok(KeyCode::F7),
        "F8" => Ok(KeyCode::F8),
        "F9" => Ok(KeyCode::F9),
        "F10" => Ok(KeyCode::F10),
        "F11" => Ok(KeyCode::F11),
        "F12" => Ok(KeyCode::F12),
        _ => Err(PixieError::Config(format!("Invalid function key: {}", s))),
    }
}

fn launch_agent_path() -> PathBuf {
    let mut path = dirs::home_dir().unwrap_or_else(|| PathBuf::from("~"));
    path.push("Library");
    path.push("LaunchAgents");
    path.push("com.pixie.plist");
    path
}

const LAUNCH_AGENT_PLIST: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.pixie</string>
    <key>ProgramArguments</key>
    <array>
        <string>/Applications/Pixie.app/Contents/MacOS/Pixie</string>
        <string>--headless</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <false/>
</dict>
</plist>
"#;

pub fn is_autostart_enabled() -> bool {
    let path = launch_agent_path();

    if !path.exists() {
        return false;
    }

    let output = Command::new("launchctl")
        .args(["list", "com.pixie"])
        .output();

    match output {
        Ok(output) => output.status.success(),
        Err(_) => false,
    }
}

pub fn set_autostart(enabled: bool) -> Result<()> {
    let path = launch_agent_path();

    if enabled {
        let parent = path.parent().ok_or_else(|| {
            PixieError::Config("Could not determine LaunchAgents directory".to_string())
        })?;

        fs::create_dir_all(parent).map_err(|e| {
            PixieError::Config(format!("Failed to create LaunchAgents directory: {}", e))
        })?;

        fs::write(&path, LAUNCH_AGENT_PLIST)
            .map_err(|e| PixieError::Config(format!("Failed to write LaunchAgent plist: {}", e)))?;

        let output = Command::new("launchctl")
            .args(["load", "-w"])
            .arg(&path)
            .output()
            .map_err(|e| PixieError::Config(format!("Failed to run launchctl load: {}", e)))?;

        if !output.status.success() {
            return Err(PixieError::Config(format!(
                "launchctl load failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }
    } else if path.exists() {
        let _ = Command::new("launchctl")
            .args(["unload", "-w"])
            .arg(&path)
            .output();

        fs::remove_file(&path).map_err(|e| {
            PixieError::Config(format!("Failed to remove LaunchAgent plist: {}", e))
        })?;
    }

    Ok(())
}

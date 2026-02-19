//! Configuration management for Pixie
//!
//! Handles TOML config file parsing and LaunchAgent management for autostart.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use global_hotkey::hotkey::{Code, Modifiers};
use serde::{Deserialize, Serialize};

use crate::error::{PixieError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_leader_key")]
    pub leader_key: String,

    #[serde(default)]
    pub autostart: bool,
}

fn default_leader_key() -> String {
    "cmd+shift+a".to_string()
}

impl Default for Config {
    fn default() -> Self {
        Config {
            leader_key: default_leader_key(),
            autostart: false,
        }
    }
}

fn config_path() -> PathBuf {
    let mut path = dirs::config_local_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("pixie");
    path.push("config.toml");
    path
}

pub fn load() -> Config {
    let path = config_path();

    match fs::read_to_string(&path) {
        Ok(content) => toml::from_str(&content).unwrap_or_default(),
        Err(_) => Config::default(),
    }
}

pub fn parse_leader_key(key: &str) -> Result<(Option<Modifiers>, Code)> {
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

fn parse_key_code(s: &str) -> Result<Code> {
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

fn char_to_code(c: char) -> Result<Code> {
    match c {
        'a' => Ok(Code::KeyA),
        'b' => Ok(Code::KeyB),
        'c' => Ok(Code::KeyC),
        'd' => Ok(Code::KeyD),
        'e' => Ok(Code::KeyE),
        'f' => Ok(Code::KeyF),
        'g' => Ok(Code::KeyG),
        'h' => Ok(Code::KeyH),
        'i' => Ok(Code::KeyI),
        'j' => Ok(Code::KeyJ),
        'k' => Ok(Code::KeyK),
        'l' => Ok(Code::KeyL),
        'm' => Ok(Code::KeyM),
        'n' => Ok(Code::KeyN),
        'o' => Ok(Code::KeyO),
        'p' => Ok(Code::KeyP),
        'q' => Ok(Code::KeyQ),
        'r' => Ok(Code::KeyR),
        's' => Ok(Code::KeyS),
        't' => Ok(Code::KeyT),
        'u' => Ok(Code::KeyU),
        'v' => Ok(Code::KeyV),
        'w' => Ok(Code::KeyW),
        'x' => Ok(Code::KeyX),
        'y' => Ok(Code::KeyY),
        'z' => Ok(Code::KeyZ),
        _ => Err(PixieError::Config(format!("Invalid letter key: {}", c))),
    }
}

fn digit_to_code(c: char) -> Result<Code> {
    match c {
        '0' => Ok(Code::Digit0),
        '1' => Ok(Code::Digit1),
        '2' => Ok(Code::Digit2),
        '3' => Ok(Code::Digit3),
        '4' => Ok(Code::Digit4),
        '5' => Ok(Code::Digit5),
        '6' => Ok(Code::Digit6),
        '7' => Ok(Code::Digit7),
        '8' => Ok(Code::Digit8),
        '9' => Ok(Code::Digit9),
        _ => Err(PixieError::Config(format!("Invalid digit key: {}", c))),
    }
}

fn function_key_to_code(s: &str) -> Result<Code> {
    match s.to_uppercase().as_str() {
        "F1" => Ok(Code::F1),
        "F2" => Ok(Code::F2),
        "F3" => Ok(Code::F3),
        "F4" => Ok(Code::F4),
        "F5" => Ok(Code::F5),
        "F6" => Ok(Code::F6),
        "F7" => Ok(Code::F7),
        "F8" => Ok(Code::F8),
        "F9" => Ok(Code::F9),
        "F10" => Ok(Code::F10),
        "F11" => Ok(Code::F11),
        "F12" => Ok(Code::F12),
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
    } else {
        if path.exists() {
            let _ = Command::new("launchctl")
                .args(["unload", "-w"])
                .arg(&path)
                .output();

            fs::remove_file(&path).map_err(|e| {
                PixieError::Config(format!("Failed to remove LaunchAgent plist: {}", e))
            })?;
        }
    }

    Ok(())
}

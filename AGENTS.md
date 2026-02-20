# AGENTS

## Project Overview
- Pixie is a macOS window focus tool written in Rust.
- Features a leader key system for keyboard-driven window management with multi-window support.
- Main entrypoint: `src/main.rs`.
- Accessibility/window logic: `src/accessibility.rs` and `src/window.rs`.
- Leader key state machine: `src/leader_mode.rs`.
- Hotkey management: `src/hotkey.rs`.
- Notification system: `src/notification.rs`.

## Development Commands
- Run tests: `cargo test`
- Run app locally: `cargo run`
- Build release binary: `cargo build --release`
- Build app bundle: `cargo bundle --release`

## App Bundle + Signing
- Uses `cargo-bundle` for app bundling (install with `just install-bundler` or `cargo install cargo-bundle`).
- Bundle configuration is in `Cargo.toml` under `[package.metadata.bundle]`.
- Custom Info.plist entries (LSUIElement, NSAppleEventsUsageDescription) are in `Info.plist.ext`.
- Build/sign via Just:
  - Ad-hoc signing (default): `just signed-app`
  - Explicit identity: `just signed-app identity="Developer ID Application: Your Name (TEAMID)"`
- Output bundle: `dist/Pixie.app`

## Dependencies
- `global-hotkey` for global hotkey registration (leader key and letter keys).

## Implementation Notes
- Keep changes minimal and focused.
- Preserve existing CLI behavior unless explicitly requested.
- If changing Accessibility behavior, verify with runtime testing on macOS.
- Leader key state machine is implemented in `leader_mode.rs` with states: Idle, Listening.
- Hotkey system uses dynamic registration:
  - Leader key (default: Cmd+Shift+A) is registered at startup.
  - Letter hotkeys (a-z, Shift+A-Z) are only registered when in listening mode.
  - Letter hotkeys are unregistered when exiting listening mode (key press or timeout).
  - This prevents letter keys from being blocked when Pixie is idle.
- Multi-window storage uses `HashMap<char, SavedWindow>` for saving/focusing windows by slot key.
- Persistence file: `saved_windows.json` in the user's config directory.
- Notification system uses `osascript` to display macOS notifications.

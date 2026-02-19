# AGENTS

## Project Overview
- Pixie is a macOS window focus tool written in Rust.
- Features a leader key system for keyboard-driven window management with multi-window support.
- Main entrypoint: `src/main.rs`.
- Accessibility/window logic: `src/accessibility.rs` and `src/window.rs`.
- Leader key state machine: `src/leader_mode.rs`.
- Notification system: `src/notification.rs`.

## Development Commands
- Run tests: `cargo test`
- Run app locally: `cargo run`
- Build release binary: `cargo build --release`

## App Bundle + Signing
- Build/sign via Just:
  - Ad-hoc signing (default): `just signed-app`
  - Explicit identity: `just signed-app identity="Developer ID Application: Your Name (TEAMID)"`
- Output bundle: `dist/Pixie.app`

## Dependencies
- `rdev` with `unstable_grab` feature for global keyboard event grabbing.

## Implementation Notes
- Keep changes minimal and focused.
- Preserve existing CLI behavior unless explicitly requested.
- If changing Accessibility behavior, verify with runtime testing on macOS.
- Leader key state machine is implemented in `leader_mode.rs` with states: Idle, Listening.
- Multi-window storage uses `HashMap<char, SavedWindow>` for saving/focusing windows by slot key.
- Persistence file: `saved_windows.json` in the user's config directory.
- Notification system uses `osascript` to display macOS notifications.

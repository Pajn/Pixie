# AGENTS

## Project Overview
- Pixie is a macOS window management tool written in Rust.
- Features a leader key system for keyboard-driven window management with multi-window support.
- Includes a GPUI-based window picker for multi-window tiling and search.
- Includes window manipulation actions: minimize, maximize, fullscreen, center, and monitor movement.
- Main entrypoint: `src/main.rs`.
- Accessibility/window logic: `src/accessibility.rs` and `src/window.rs`.
- Leader key state machine: `src/leader_mode.rs`.
- Event-tap + leader input routing: `src/event_tap.rs`.
- Window picker UI: `src/ui/window_picker.rs`.
- Notification system: `src/notification.rs`.
- Configuration: `src/config.rs` defines the `Action` enum and parses keybinds.

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
- `gpui` for the window picker UI.
- CoreGraphics event tap + Accessibility APIs for global key handling and window control.

## Implementation Notes
- Keep changes minimal and focused.
- Preserve existing CLI behavior unless explicitly requested.
- If changing Accessibility behavior, verify with runtime testing on macOS.
- Leader key state machine is implemented in `leader_mode.rs` with states: Idle, Listening.
- Global key handling is done via an event tap (`event_tap.rs`) and routed into UI actions in `main.rs`.
- Multi-window storage uses `HashMap<char, SavedWindow>` for saving/focusing windows by slot key.
- Persistence file: `saved_windows.json` in the user's config directory.
- Notification system uses `osascript` to display macOS notifications.
- The `Action` enum in `config.rs` defines all available actions: focus_*, minimize, maximize, fullscreen, center, move_monitor_*, tile, Place(String).
- Window manipulation functions are in `accessibility.rs`: `minimize_window`, `maximize_window`, `toggle_fullscreen`, `center_window`, `apply_placement`.
- The `Placement` struct and builtin placements are defined in `config.rs`.
- Monitor movement functions in `accessibility.rs`: `move_window_to_monitor`, `get_all_screens`, and `Screen` struct for monitor detection.
- Move monitor logic preserves relative window position by calculating percentage-based coordinates across screens.
- Window picker behavior:
  - Triggered by the `tile` action.
  - Lists current-monitor windows first, then other/minimized windows with a separator.
  - Supports vim-style search (`/`, then `n`/`N` navigation).
  - Auto-dismisses when the picker loses focus.

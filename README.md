# Pixie 🧚

A lightweight macOS window focusing tool with global shortcuts and multi-window support. Register windows to letter slots (a-z), then instantly focus them from anywhere using the leader key system.

## Features

- **Leader Key System**: ⌘⇧A activates leader mode for quick window operations
- **Multi-Window Support**: 26 slots (a-z) for saving and focusing multiple windows
- **Global Hotkeys**: Register and focus windows from anywhere in macOS
- **macOS Notifications**: Visual feedback for window registration and focus actions
- **Menu Bar App**: Optional status bar icon for quick access
- **CLI Support**: Use from the command line for scripting
- **Persistence**: Saved windows survive app restarts
- **Lightweight**: Minimal resource usage
- **Configurable Hotkeys**: Customize the leader key via config file
- **Auto-start**: Optionally launch Pixie at login

## Installation

### From Source

```bash
git clone <repository-url>
cd pixie
cargo build --release
```

The binary will be at `target/release/pixie`.

## Usage

### First Run

On first run, Pixie will request Accessibility permissions. You must grant these for Pixie to work:

1. Open **System Preferences** → **Privacy & Security** → **Accessibility**
2. Enable **Pixie** in the list
3. Restart Pixie

### Running the App

```bash
# Run with menu bar icon (default)
./pixie

# Run without UI (headless daemon mode)
./pixie --headless
```

### Keyboard Shortcuts

Pixie uses a leader key system. Press **⌘⇧A** (Cmd+Shift+A) to enter leader mode, then:

| Key | Action |
|-----|--------|
| `a-z` | Focus the window registered at that letter slot |
| `A-Z` (Shift+a-z) | Register the currently focused window to that slot |
| `Escape` | Cancel leader mode |
| *(configurable timeout, default 2 seconds)* | Leader mode auto-cancels after timeout |

**Examples:**
- `⌘⇧A` then `f` → Focus window at slot 'f'
- `⌘⇧A` then `Shift+m` → Register current window to slot 'm'

### CLI Commands

```bash
# Register the current window to a specific letter slot
./pixie register <slot>

# Focus the window at a specific letter slot
./pixie focus <slot>

# Show all saved windows
./pixie show

# Clear a specific slot or all slots
./pixie clear [slot]
```

**Examples:**
```bash
./pixie register a    # Register current window to slot 'a'
./pixie focus a       # Focus window at slot 'a'
./pixie show          # List all saved windows
./pixie clear a       # Clear slot 'a'
./pixie clear         # Clear all slots
```

### Menu Bar

When running in menu bar mode, you can:

- Click the 🧚 icon to see options
- View all saved windows by slot
- Register current window to a slot
- Focus a saved window
- Clear slots
- Quit

## How It Works

1. **Leader Mode**: Press `⌘⇧A` to enter leader mode. Pixie listens for the next keypress (with a configurable timeout, default 2 seconds).

2. **Register**: Press a letter key with Shift (e.g., `Shift+m`) to register the currently focused window to that slot. Pixie captures the window using the macOS Accessibility API and stores its PID and CGWindowID.

3. **Focus**: Press a letter key without Shift (e.g., `m`) to focus the window registered at that slot. Pixie finds the window by its stored identifiers and brings it to the front by:
   - Setting the application's `AXFrontmost` attribute to true
   - Setting the window's `AXMain` attribute to true
   - Performing the `AXRaise` action

4. **Notifications**: Pixie shows macOS notifications for successful registrations, focus actions, and errors.

## Configuration

Pixie can be configured via a TOML config file at `~/Library/Application Support/pixie/config.toml`.

### Config File Example

```toml
# Leader key (modifiers + key, separated by +)
# Modifiers: cmd/super, alt/option, shift, ctrl/control
# Keys: a-z, 0-9, F1-F12, space, escape, enter, tab, etc.
leader_key = "cmd+shift+a"

# Auto-start Pixie on login
autostart = false

# Leader mode timeout in seconds (how long to wait for a letter key after pressing leader)
timeout = 2
```

### Leader Key Options

**Modifiers:**
- `cmd` or `super` - Command (⌘) key
- `alt` or `option` - Option (⌥) key
- `shift` - Shift (⇧) key
- `ctrl` or `control` - Control (^) key

**Keys:**
- Letters: `a` through `z`
- Numbers: `0` through `9`
- Function keys: `f1` through `f12`
- Special keys: `space`, `escape` (or `esc`), `enter` (or `return`), `tab`, `backspace`, `delete`, `insert`, `home`, `end`, `pageup`, `pagedown`, `up`, `down`, `left`, `right`

### Example Configurations

```toml
# Use Cmd+Escape as leader (no conflict with Spotlight)
leader_key = "cmd+escape"
autostart = true
timeout = 3
```

```toml
# Use F13 as leader with longer timeout
leader_key = "f13"
autostart = true
timeout = 5
```

### Data Storage

- Config file: `~/Library/Application Support/pixie/config.toml`
- Saved windows: `~/Library/Application Support/pixie/saved_windows.json`
- LaunchAgent (for autostart): `~/Library/LaunchAgents/com.pixie.plist`

## Requirements

- macOS 10.15 or later
- Accessibility permissions must be granted

## License

MIT License

# Pixie 🧚

A lightweight macOS window management tool with global shortcuts and multi-window support. Register windows to letter slots (a-z), then instantly focus them from anywhere using the leader key system. Includes window manipulation actions like minimize, maximize, fullscreen, center, and monitor movement.

## Features

- **Leader Key System**: ⌘⇧A activates leader mode for quick window operations
- **Multi-Window Support**: 26 slots (a-z) for saving and focusing multiple windows
- **Window Management**: Minimize, maximize, fullscreen, center, and move windows between monitors
- **Window Picker (GPUI)**: Interactive picker UI for selecting/tile multiple windows
- **Directional Focus**: Navigate windows by direction (left, right, up, down)
- **Global Hotkeys**: Register and focus windows from anywhere in macOS
- **macOS Notifications**: Visual feedback for window registration and focus actions
- **Menu Bar App**: Optional status bar icon for quick access
- **CLI Support**: Use from the command line for scripting
- **Persistence**: Saved windows survive app restarts
- **Lightweight**: Minimal resource usage
- **Configurable Hotkeys**: Customize the leader key and all keybinds via config file
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

### Window Management Actions

Pixie provides window manipulation actions that can be bound to keys in your config:

| Action | Description |
|--------|-------------|
| `minimize` | Minimize the focused window |
| `maximize` | Maximize the focused window (fill screen without fullscreen mode) |
| `fullscreen` | Toggle fullscreen mode for the focused window |
| `center` | Center the focused window on screen |
| `move_monitor_left` | Move window to the monitor on the left |
| `move_monitor_right` | Move window to the monitor on the right |
| `move_monitor_up` | Move window to the monitor above |
| `move_monitor_down` | Move window to the monitor below |
| `focus_left` | Focus the window to the left |
| `focus_right` | Focus the window to the right |
| `focus_up` | Focus the window above |
| `focus_down` | Focus the window below |
| `tile` | Open the window picker and tile selected windows on the current monitor |
| `place_<name>` | Place window using a builtin or custom placement |

These actions have no default shortcuts. Configure them in your `config.toml` under `[keybinds]`.

### Window Picker

The `tile` action opens a GPUI-powered picker that lists windows on the current monitor first, then other-monitor/minimized windows.

Default picker controls:
- `j` / `k` (or arrow keys): Move focus
- `space`: Toggle selection
- `enter`: Tile selected windows
- `esc`: Close picker

Search controls (vim-style):
- `/`: Enter search input mode
- Type to filter by app name/title
- `enter` / `esc`: Exit search input mode
- `n` / `N`: Jump to next/previous match

### Builtin Placements

| Placement | Position | Size |
|-----------|----------|------|
| `left` | Left half | 50% width, 100% height |
| `right` | Right half | 50% width, 100% height |
| `top` | Top half | 100% width, 50% height |
| `bottom` | Bottom half | 100% width, 50% height |
| `top_left` | Top-left quarter | 50% width, 50% height |
| `top_right` | Top-right quarter | 50% width, 50% height |
| `bottom_left` | Bottom-left quarter | 50% width, 50% height |
| `bottom_right` | Bottom-right quarter | 50% width, 50% height |
| `center` | Centered (keeps window size) | Unchanged |

## Window Placements

Placements position and size windows using screen percentages. Use them with the `place_<name>` action format:

```toml
[keybinds]
"leader+h" = "place_left"
```

### Custom Placements

Define custom placements in your config:

```toml
[placements]
[placements.third_left]
left = "0%"
width = "33%"
height = "100%"

[placements.third_middle]
left = "33%"
width = "34%"
height = "100%"

[placements.third_right]
left = "67%"
width = "33%"
height = "100%"
```

Then bind them: `"leader+1" = "place_third_left"`

Placement fields:
- `top`, `left` - Position (percentage string like `"50%"`, or `"center"` to center while keeping size)
- `width`, `height` - Size (percentage string like `"50%"`)
- Omitted fields keep the window's current value

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
- Open `config.toml` in the default text editor
- See the icon highlight while leader mode is active
- Register current window to a slot
- Focus a saved window
- Clear slots
- Quit

## How It Works

1. **Leader Mode**: Press `⌘⇧A` to enter leader mode. Pixie listens for the next keypress (with a configurable timeout, default 2 seconds).

2. **Register**: Press a letter key with Shift (e.g., `Shift+m`) to register the currently focused window to that slot. Pixie captures the window using the macOS Accessibility API and stores its PID and CGWindowID.

3. **Focus**: Press a letter key without Shift (e.g., `m`) to focus the window registered at that slot. Pixie first tries the exact saved window, then any window from the same app, and if none are open it launches the app and focuses the first available window.
   It brings windows to the front by:
   - Setting the application's `AXFrontmost` attribute to true
   - Setting the window's `AXMain` attribute to true
   - Performing the `AXRaise` action

4. **Notifications**: Pixie shows macOS notifications for successful registrations, focus actions, and errors.

## Configuration

Pixie can be configured via a TOML config file at `~/Library/Application Support/pixie/config.toml`.
Pixie watches this file and applies `leader_key`, `timeout`, `autostart`, and `[keybinds]` changes automatically while running (menu bar icon/color changes still require a restart).

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

# Show Pixie in the macOS menu bar
menubar_icon = true

# Optional leader-mode icon color (#RRGGBB).
# If omitted, Pixie uses your macOS System Settings accent color.
# menubar_active_color = "#3b82f6"

[keybinds]
# Directional focus (works in leader mode)
"leader+h" = "focus_left"
"leader+l" = "focus_right"
"leader+j" = "focus_down"
"leader+k" = "focus_up"

# Window management (configure your own shortcuts)
"leader+m" = "minimize"
"leader+shift+m" = "maximize"
"leader+f" = "fullscreen"
"leader+c" = "center"
"leader+t" = "tile"

# Builtin placements
"leader+h" = "place_left"
"leader+l" = "place_right"
"leader+k" = "place_top"
"leader+j" = "place_bottom"
"leader+u" = "place_top_left"
"leader+i" = "place_top_right"
"leader+n" = "place_bottom_left"
"leader+m" = "place_bottom_right"
"leader+c" = "place_center"

# Custom placements
"leader+1" = "place_third_left"
"leader+2" = "place_third_middle"
"leader+3" = "place_third_right"

# Move window between monitors (preserves relative position)
"leader+left" = "move_monitor_left"
"leader+right" = "move_monitor_right"
"leader+up" = "move_monitor_up"
"leader+down" = "move_monitor_down"

# Direct keybinds (work without leader key)
"cmd+ctrl+m" = "minimize"
"cmd+ctrl+f" = "fullscreen"

# Custom placements
[placements]
[placements.third_left]
left = "0%"
width = "33%"
height = "100%"

[placements.third_middle]
left = "33%"
width = "34%"
height = "100%"

[placements.third_right]
left = "67%"
width = "33%"
height = "100%"
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

### Keybind Format

Keybinds use the format `"modifiers+key" = "action"`:

- **Leader keybinds**: Prefix with `leader+` (e.g., `"leader+m"` means press leader key then `m`)
- **Direct keybinds**: No prefix (e.g., `"cmd+ctrl+m"` works globally without leader mode)
- **Shift modifier**: Use `shift+` for uppercase (e.g., `"leader+shift+m"`)

### Monitor Movement

The `move_monitor_*` actions preserve the window's relative position when moving between monitors. For example, if a window is centered on one monitor, it will be centered on the destination monitor. This works by:

1. Calculating the window's position as a percentage of the current screen
2. Applying that same percentage to the target monitor
3. Resizing proportionally if monitors have different resolutions

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

## Editor Support

Pixie provides a JSON schema to enable autocompletion and documentation for your `config.toml`.

### VS Code (Even Better TOML)

If you use the **Even Better TOML** extension, add the following line to the top of your `config.toml`:

```toml
#:schema https://raw.githubusercontent.com/<user>/pixie/main/pixie.schema.json
```

Or if you have the repo cloned locally, you can point to the local file:

```toml
#:schema ./pixie.schema.json
```

## Requirements

- macOS 10.15 or later
- Accessibility permissions must be granted

## License

MIT License

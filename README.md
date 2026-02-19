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
| *(2 second timeout)* | Leader mode auto-cancels after 2 seconds |

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

1. **Leader Mode**: Press `⌘⇧A` to enter leader mode. Pixie listens for the next keypress (with a 2-second timeout).

2. **Register**: Press a letter key with Shift (e.g., `Shift+m`) to register the currently focused window to that slot. Pixie captures the window using the macOS Accessibility API and stores its PID and CGWindowID.

3. **Focus**: Press a letter key without Shift (e.g., `m`) to focus the window registered at that slot. Pixie finds the window by its stored identifiers and brings it to the front by:
   - Setting the application's `AXFrontmost` attribute to true
   - Setting the window's `AXMain` attribute to true
   - Performing the `AXRaise` action

4. **Notifications**: Pixie shows macOS notifications for successful registrations, focus actions, and errors.

## Configuration

Window state is persisted in `~/.config/pixie/saved_windows.json`.

## Requirements

- macOS 10.15 or later
- Accessibility permissions must be granted

## License

MIT License

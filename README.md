# Pixie 🧚

A lightweight macOS window focusing tool with global shortcuts. Register any window with a keyboard shortcut, then instantly focus it from anywhere with another shortcut.

## Features

- **Global Hotkeys**: Register and focus windows from anywhere in macOS
- **Menu Bar App**: Optional status bar icon for quick access
- **CLI Support**: Use from the command line for scripting
- **Persistence**: Saved window survives app restarts
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

| Shortcut | Action |
|----------|--------|
| `⌘⇧R` (Cmd+Shift+R) | Register the currently focused window |
| `⌘⇧F` (Cmd+Shift+F) | Focus the saved window |

### CLI Commands

```bash
# Register the current window (one-shot)
./pixie register

# Focus the saved window (one-shot)
./pixie focus

# Show info about the saved window
./pixie show

# Clear the saved window
./pixie clear
```

### Menu Bar

When running in menu bar mode, you can:

- Click the 🧚 icon to see options
- Register current window
- Focus saved window
- Show saved window info
- Clear saved window
- Quit

## How It Works

1. **Register**: When you press `⌘⇧R`, Pixie captures the currently focused window using the macOS Accessibility API. It stores the window's PID and CGWindowID.

2. **Focus**: When you press `⌘⇧F`, Pixie finds the window by its stored identifiers and brings it to the front by:
   - Setting the application's `AXFrontmost` attribute to true
   - Setting the window's `AXMain` attribute to true
   - Performing the `AXRaise` action

## Configuration

Window state is persisted in `~/.config/pixie/saved_window.json`.

## Requirements

- macOS 10.15 or later
- Accessibility permissions must be granted

## License

MIT License

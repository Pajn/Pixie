//! Pixie - macOS Window Focusing Tool

mod accessibility;
mod error;
mod hotkey;
mod leader_mode;
mod menu_bar;
mod notification;
mod window;

use clap::{Parser, Subcommand};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use error::{PixieError, Result};
use leader_mode::{LeaderModeController, LeaderModeEvent};
use window::WindowManager;

/// Pixie - macOS Window Focusing Tool
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Run without menu bar UI (daemon mode)
    #[arg(long)]
    headless: bool,

    /// Subcommand for one-shot operations
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Register the currently focused window to a slot
    Register {
        /// Slot letter (a-z)
        slot: char,
    },
    /// Focus the window at a specific slot
    Focus {
        /// Slot letter (a-z)
        slot: char,
    },
    /// Show all saved windows
    Show,
    /// Clear saved window(s)
    Clear {
        /// Slot letter (a-z), or omit to clear all
        slot: Option<char>,
    },
}

static RUNNING: AtomicBool = AtomicBool::new(true);

fn main() -> Result<()> {
    let args = Args::parse();

    // Check if we're running from Terminal - if so, Terminal needs permissions too
    let is_from_terminal = std::env::var("TERM_PROGRAM").is_ok();
    if is_from_terminal {
        println!("Note: Running from Terminal. If permissions don't work,");
        println!("      try running as: open /Applications/Pixie.app\n");
    }

    // Verify accessibility works
    let mut attempts = 0;
    loop {
        match accessibility::test_api_access() {
            Ok(()) => {
                tracing::info!("Accessibility API working");
                break;
            }
            Err(e) => {
                if attempts == 0 {
                    println!("\n⚠️  Accessibility API not available: {}", e);
                    println!("\nSteps to fix:");
                    println!("1. System Preferences → Privacy & Security → Accessibility");
                    println!("2. Make sure Pixie.app is in the list AND CHECKED");
                    println!("3. If running from Terminal, also add Terminal.app");
                    println!("\nOpening System Preferences...\n");

                    accessibility::request_accessibility_permissions();

                    let _ = std::process::Command::new("open")
                        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
                        .spawn();
                }

                attempts += 1;
                if attempts % 5 == 0 {
                    println!("Still waiting for permissions... (attempt {})", attempts);
                }

                std::thread::sleep(std::time::Duration::from_secs(1));
            }
        }
    }

    let window_manager = Arc::new(WindowManager::new()?);

    if let Some(cmd) = args.command {
        return handle_command(cmd, &window_manager);
    }

    run_daemon(window_manager, args.headless)
}

fn handle_command(cmd: Commands, window_manager: &WindowManager) -> Result<()> {
    match cmd {
        Commands::Register { slot } => {
            let slot = slot.to_ascii_lowercase();
            if !slot.is_ascii_lowercase() {
                return Err(PixieError::Config(format!(
                    "Slot must be a letter a-z, got '{}'",
                    slot
                )));
            }
            let (_, window) = window_manager.register_current_window(slot)?;
            let display = window.display_string();
            notification::notify(
                "Pixie",
                &format!("Registered to [{}]: {}", slot, window.app_name),
            );
            println!("✓ Registered to slot '{}': {}", slot, display);
        }
        Commands::Focus { slot } => {
            let slot = slot.to_ascii_lowercase();
            let window = window_manager.focus_saved_window(slot)?;
            notification::notify("Pixie", &format!("Focused [{}]: {}", slot, window.app_name));
            println!("✓ Focused slot '{}': {}", slot, window.display_string());
        }
        Commands::Show => {
            let windows = window_manager.get_all_saved_windows();
            if windows.is_empty() {
                println!("No windows saved. Use 'pixie register <slot>' to save one.");
            } else {
                println!("Saved windows:");
                for (slot, window) in windows {
                    println!("  [{}] {}", slot, window.display_string());
                }
            }
        }
        Commands::Clear { slot } => match slot {
            Some(s) => {
                let s = s.to_ascii_lowercase();
                if window_manager.clear_slot(s)? {
                    notification::notify("Pixie", &format!("Cleared [{}]", s));
                    println!("✓ Cleared slot '{}'", s);
                } else {
                    println!("Slot '{}' was empty", s);
                }
            }
            None => {
                window_manager.clear_all_windows()?;
                notification::notify("Pixie", "Cleared all slots");
                println!("✓ Cleared all saved windows");
            }
        },
    }

    Ok(())
}

fn run_daemon(window_manager: Arc<WindowManager>, headless: bool) -> Result<()> {
    println!("🧚 Pixie started");
    println!("  ⌘⇧A - Leader key (then press a letter to focus, or Shift+letter to register)");

    let windows = window_manager.get_all_saved_windows();
    if windows.is_empty() {
        println!("  No windows saved.");
    } else {
        println!("  Saved windows:");
        for (slot, window) in windows {
            println!("    [{}] {}", slot, window.display_string());
        }
    }

    ctrlc::set_handler(|| {
        println!("\nShutting down...");
        RUNNING.store(false, Ordering::SeqCst);
    })
    .map_err(|e| PixieError::Config(format!("Failed to set Ctrl+C handler: {}", e)))?;

    let leader_mode_controller = Arc::new(LeaderModeController::new()?);
    let hotkey_manager = hotkey::HotkeyManager::new()?;
    let leader_id = hotkey_manager.leader_id;

    let receiver = global_hotkey::GlobalHotKeyEvent::receiver();
    let controller_for_hotkey = Arc::clone(&leader_mode_controller);
    let wm_for_events = Arc::clone(&window_manager);
    let event_receiver = leader_mode_controller.events();

    std::thread::spawn(move || loop {
        if !RUNNING.load(Ordering::SeqCst) {
            break;
        }

        if let Ok(event) = receiver.try_recv() {
            if event.state == global_hotkey::HotKeyState::Pressed && event.id == leader_id {
                controller_for_hotkey.enter_listening_mode();
                notification::notify("Pixie", "Listening...");
                println!("Listening...");
            }
        }

        if let Ok(event) = event_receiver.try_recv() {
            match event {
                LeaderModeEvent::RegisterSlot(c) => {
                    let slot = c.to_ascii_lowercase();
                    match wm_for_events.register_current_window(slot) {
                        Ok((_, window)) => {
                            notification::notify(
                                "Pixie",
                                &format!("Registered to [{}]: {}", slot, window.app_name),
                            );
                            println!("✓ Registered to [{}]: {}", slot, window.display_string())
                        }
                        Err(e) => eprintln!("✗ Failed: {}", e),
                    }
                }
                LeaderModeEvent::FocusSlot(c) => match wm_for_events.focus_saved_window(c) {
                    Ok(window) => {
                        notification::notify(
                            "Pixie",
                            &format!("Focused [{}]: {}", c, window.app_name),
                        );
                        println!("✓ Focused [{}]: {}", c, window.display_string())
                    }
                    Err(e) => eprintln!("✗ Failed: {}", e),
                },
                LeaderModeEvent::Cancelled => {
                    notification::notify("Pixie", "Cancelled");
                    println!("Cancelled");
                }
            }
        }

        std::thread::sleep(std::time::Duration::from_millis(50));
    });

    if headless {
        println!("Running in headless mode (Ctrl+C to quit)...");
        while RUNNING.load(Ordering::SeqCst) {
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    } else {
        run_with_menu_bar(&window_manager)?;
    }

    Ok(())
}

#[cfg(target_os = "macos")]
fn run_with_menu_bar(window_manager: &Arc<WindowManager>) -> Result<()> {
    use cocoa::appkit::{NSApplication, NSStatusBar};
    use cocoa::base::{id, nil};
    use cocoa::foundation::NSString;
    use objc::{class, msg_send, sel, sel_impl};

    unsafe {
        let app = NSApplication::sharedApplication(nil);
        app.setActivationPolicy_(
            cocoa::appkit::NSApplicationActivationPolicy::NSApplicationActivationPolicyAccessory,
        );

        let status_bar = NSStatusBar::systemStatusBar(nil);
        let status_item: id = msg_send![status_bar, statusItemWithLength: -1.0f64];

        let button: id = msg_send![status_item, button];
        let title = NSString::alloc(nil).init_str("🧚");
        let _: () = msg_send![button, setTitle: title];

        let menu: id = msg_send![class!(NSMenu), alloc];
        let _: () = msg_send![menu, init];

        let quit_title = NSString::alloc(nil).init_str("Quit Pixie");
        let quit_item: id = msg_send![class!(NSMenuItem), alloc];
        let quit_key = NSString::alloc(nil).init_str("q");
        let _: () = msg_send![quit_item, initWithTitle: quit_title action: sel!(terminate:) keyEquivalent: quit_key];
        let _: () = msg_send![menu, addItem: quit_item];

        let _: () = msg_send![status_item, setMenu: menu];
        std::mem::forget(status_item);

        app.activateIgnoringOtherApps_(true);
        let _wm = window_manager.clone();
        std::mem::forget(_wm);
    }

    println!("Menu bar icon active. Quit from menu or Ctrl+C.");

    unsafe {
        let app = NSApplication::sharedApplication(nil);
        app.run();
    }

    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn run_with_menu_bar(_window_manager: &Arc<WindowManager>) -> Result<()> {
    Err(PixieError::MenuBar(
        "Menu bar mode is only supported on macOS".to_string(),
    ))
}

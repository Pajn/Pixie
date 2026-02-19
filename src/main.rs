//! Pixie - macOS Window Focusing Tool

mod accessibility;
mod error;
mod hotkey;
mod menu_bar;
mod window;

use clap::{Parser, Subcommand};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use error::{PixieError, Result};
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
    /// Register the currently focused window
    Register,
    /// Focus the saved window
    Focus,
    /// Show information about the saved window
    Show,
    /// Clear the saved window
    Clear,
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
        Commands::Register => {
            let window = window_manager.register_current_window()?;
            println!("✓ Registered window: {}", window.display_string());
        }
        Commands::Focus => {
            let window = window_manager.focus_saved_window()?;
            println!("✓ Focused window: {}", window.display_string());
        }
        Commands::Show => match window_manager.get_saved_window() {
            Some(window) => {
                println!("Saved window: {}", window.display_string());
                println!("  PID: {}", window.pid);
                println!("  Window ID: {}", window.window_id);
            }
            None => {
                println!("No window saved. Use 'pixie register' to save one.");
            }
        },
        Commands::Clear => {
            window_manager.clear_saved_window()?;
            println!("✓ Cleared saved window");
        }
    }

    Ok(())
}

fn run_daemon(window_manager: Arc<WindowManager>, headless: bool) -> Result<()> {
    println!("🧚 Pixie started");
    println!("  ⌘⇧R - Register current window");
    println!("  ⌘⇧A - Focus saved window");

    if let Some(window) = window_manager.get_saved_window() {
        println!("  Saved: {}", window.display_string());
    }

    ctrlc::set_handler(|| {
        println!("\nShutting down...");
        RUNNING.store(false, Ordering::SeqCst);
    })
    .map_err(|e| PixieError::Config(format!("Failed to set Ctrl+C handler: {}", e)))?;

    let hotkey_manager = hotkey::HotkeyManager::new()?;
    let register_id = hotkey_manager.register_id;
    let focus_id = hotkey_manager.focus_id;

    let receiver = global_hotkey::GlobalHotKeyEvent::receiver();
    let wm_clone = Arc::clone(&window_manager);

    std::thread::spawn(move || loop {
        if !RUNNING.load(Ordering::SeqCst) {
            break;
        }

        if let Ok(event) = receiver.try_recv() {
            if event.state == global_hotkey::HotKeyState::Pressed {
                if event.id == register_id {
                    match wm_clone.register_current_window() {
                        Ok(window) => println!("✓ Registered: {}", window.display_string()),
                        Err(e) => eprintln!("✗ Failed: {}", e),
                    }
                } else if event.id == focus_id {
                    match wm_clone.focus_saved_window() {
                        Ok(window) => println!("✓ Focused: {}", window.display_string()),
                        Err(e) => eprintln!("✗ Failed: {}", e),
                    }
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

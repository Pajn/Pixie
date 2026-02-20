//! Pixie - macOS Window Focusing Tool

mod accessibility;
mod config;
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
    let config = config::load();

    if config.autostart && !config::is_autostart_enabled() {
        if let Err(e) = config::set_autostart(true) {
            eprintln!("Warning: Failed to enable autostart: {}", e);
        }
    } else if !config.autostart && config::is_autostart_enabled() {
        if let Err(e) = config::set_autostart(false) {
            eprintln!("Warning: Failed to disable autostart: {}", e);
        }
    }

    let leader = config::parse_leader_key(&config.leader_key).unwrap_or_else(|_| {
        (
            Some(global_hotkey::hotkey::Modifiers::SUPER | global_hotkey::hotkey::Modifiers::SHIFT),
            global_hotkey::hotkey::Code::KeyA,
        )
    });

    println!("🧚 Pixie started");
    println!(
        "  {} - Leader key (then press a letter to focus, or Shift+letter to register)",
        config.leader_key
    );

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

    let leader_mode_controller = Arc::new(LeaderModeController::with_timeout(
        std::time::Duration::from_secs(config.timeout),
    )?);
    let hotkey_config = hotkey::HotkeyConfig { leader };
    let hotkey_manager = Arc::new(hotkey::HotkeyManager::with_config(hotkey_config)?);
    let leader_id = hotkey_manager.leader_id;

    let receiver = global_hotkey::GlobalHotKeyEvent::receiver();
    let controller_for_hotkey = Arc::clone(&leader_mode_controller);
    let hotkey_manager_for_thread = Arc::clone(&hotkey_manager);
    let wm_for_events = Arc::clone(&window_manager);
    let event_receiver = leader_mode_controller.events();

    std::thread::spawn(move || loop {
        if !RUNNING.load(Ordering::SeqCst) {
            break;
        }

        if let Ok(event) = receiver.try_recv() {
            if event.state == global_hotkey::HotKeyState::Pressed {
                if event.id == leader_id {
                    hotkey_manager_for_thread.register_letter_hotkeys().ok();
                    controller_for_hotkey.enter_listening_mode();
                    notification::notify("Pixie", "Listening...");
                    println!("Listening...");
                } else if let Some((letter, has_shift)) =
                    hotkey_manager_for_thread.get_letter_info(event.id)
                {
                    if controller_for_hotkey.is_listening() {
                        hotkey_manager_for_thread.unregister_letter_hotkeys().ok();
                        controller_for_hotkey.handle_key(letter, has_shift);
                    }
                }
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
                    hotkey_manager_for_thread.unregister_letter_hotkeys().ok();
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
    use cocoa::base::{id, nil, YES};
    use cocoa::foundation::NSString;
    use objc::declare::ClassDecl;
    use objc::runtime::{Object, Sel};
    use objc::{class, msg_send, sel, sel_impl};
    use std::ffi::c_void;
    use std::sync::atomic::{AtomicBool, Ordering};

    static CLASS_CREATED: AtomicBool = AtomicBool::new(false);

    extern "C" fn noop_imp(_this: &Object, _sel: Sel, _sender: id) {}

    extern "C" fn clear_all_windows_imp(this: &Object, _sel: Sel, _sender: id) {
        unsafe {
            let wm_ptr: *mut c_void = *this.get_ivar("windowManager");
            let wm = wm_ptr as *const WindowManager;
            if let Some(wm) = wm.as_ref() {
                let _ = wm.clear_all_windows();
                let _ = crate::notification::notify("Pixie", "Cleared all slots");
            }
        }
    }

    extern "C" fn menu_needs_update_imp(this: &Object, _sel: Sel, menu: id) {
        unsafe {
            let _: () = msg_send![menu, removeAllItems];

            let wm_ptr: *mut c_void = *this.get_ivar("windowManager");
            let wm = wm_ptr as *const WindowManager;
            let windows = if let Some(wm) = wm.as_ref() {
                wm.get_all_saved_windows()
            } else {
                std::collections::HashMap::new()
            };

            let header_title = NSString::alloc(nil).init_str("Saved Windows");
            let header_item: id = msg_send![class!(NSMenuItem), alloc];
            let empty_str = NSString::alloc(nil).init_str("");
            let _: () = msg_send![header_item, initWithTitle: header_title action: sel!(noop:) keyEquivalent: empty_str];
            let _: () = msg_send![header_item, setEnabled: false];
            let _: () = msg_send![menu, addItem: header_item];

            if windows.is_empty() {
                let no_windows_title = NSString::alloc(nil).init_str("No windows saved");
                let no_windows_item: id = msg_send![class!(NSMenuItem), alloc];
                let empty_str = NSString::alloc(nil).init_str("");
                let _: () = msg_send![no_windows_item, initWithTitle: no_windows_title action: sel!(noop:) keyEquivalent: empty_str];
                let _: () = msg_send![no_windows_item, setEnabled: false];
                let _: () = msg_send![menu, addItem: no_windows_item];
            } else {
                let mut slots: Vec<char> = windows.keys().cloned().collect();
                slots.sort();
                for slot in slots {
                    if let Some(win) = windows.get(&slot) {
                        let display = format!("[{}] {} - {}", slot, win.app_name, win.title);
                        let item_title = NSString::alloc(nil).init_str(&display);
                        let item: id = msg_send![class!(NSMenuItem), alloc];
                        let empty_str = NSString::alloc(nil).init_str("");
                        let _: () = msg_send![item, initWithTitle: item_title action: sel!(noop:) keyEquivalent: empty_str];
                        let _: () = msg_send![item, setEnabled: false];
                        let _: () = msg_send![menu, addItem: item];
                    }
                }
            }

            let sep: id = msg_send![class!(NSMenuItem), separatorItem];
            let _: () = msg_send![menu, addItem: sep];

            let clear_title = NSString::alloc(nil).init_str("Clear All Slots");
            let clear_item: id = msg_send![class!(NSMenuItem), alloc];
            let empty_str = NSString::alloc(nil).init_str("");
            let delegate: id = this as *const Object as id;
            let _: () = msg_send![clear_item, initWithTitle: clear_title action: sel!(clearAll:) keyEquivalent: empty_str];
            let _: () = msg_send![clear_item, setTarget: delegate];
            let _: () = msg_send![menu, addItem: clear_item];

            let sep2: id = msg_send![class!(NSMenuItem), separatorItem];
            let _: () = msg_send![menu, addItem: sep2];

            let quit_title = NSString::alloc(nil).init_str("Quit Pixie");
            let quit_item: id = msg_send![class!(NSMenuItem), alloc];
            let quit_key = NSString::alloc(nil).init_str("q");
            let _: () = msg_send![quit_item, initWithTitle: quit_title action: sel!(terminate:) keyEquivalent: quit_key];
            let _: () = msg_send![menu, addItem: quit_item];
        }
    }

    unsafe {
        let delegate_class = if CLASS_CREATED.swap(true, Ordering::SeqCst) {
            class!(PixieMenuDelegate)
        } else {
            let superclass = class!(NSObject);
            let mut decl = ClassDecl::new("PixieMenuDelegate", superclass)
                .expect("Failed to create PixieMenuDelegate class");
            decl.add_ivar::<*mut c_void>("windowManager");
            decl.add_method(sel!(noop:), noop_imp as extern "C" fn(&Object, Sel, id));
            decl.add_method(
                sel!(clearAll:),
                clear_all_windows_imp as extern "C" fn(&Object, Sel, id),
            );
            decl.add_method(
                sel!(menuNeedsUpdate:),
                menu_needs_update_imp as extern "C" fn(&Object, Sel, id),
            );
            decl.register()
        };

        let delegate: id = msg_send![delegate_class, alloc];
        let delegate: id = msg_send![delegate, init];

        let wm_ptr = Arc::into_raw(window_manager.clone()) as *mut c_void;
        (*delegate).set_ivar("windowManager", wm_ptr);

        let app = NSApplication::sharedApplication(nil);
        app.setActivationPolicy_(
            cocoa::appkit::NSApplicationActivationPolicy::NSApplicationActivationPolicyAccessory,
        );

        let status_bar = NSStatusBar::systemStatusBar(nil);
        let status_item: id = msg_send![status_bar, statusItemWithLength: -1.0f64];

        let button: id = msg_send![status_item, button];

        let bundle: id = msg_send![class!(NSBundle), mainBundle];
        let resource_path: id = msg_send![bundle, resourcePath];
        let resource_path_str: *const i8 = msg_send![resource_path, UTF8String];
        let resource_path_cstr = std::ffi::CStr::from_ptr(resource_path_str);
        let image_path = format!(
            "{}/menuTemplate@2x.png",
            resource_path_cstr.to_str().unwrap()
        );
        let image_path_ns = NSString::alloc(nil).init_str(&image_path);

        let image: id = msg_send![class!(NSImage), alloc];
        let image: id = msg_send![image, initWithContentsOfFile: image_path_ns];
        let _: () = msg_send![image, setTemplate: YES];
        let _: () = msg_send![button, setImage: image];

        let menu: id = msg_send![class!(NSMenu), alloc];
        let _: () = msg_send![menu, init];
        let _: () = msg_send![menu, setDelegate: delegate];

        let _: () = msg_send![status_item, setMenu: menu];
        let _ = status_item;
        let _ = delegate;

        app.activateIgnoringOtherApps_(true);
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

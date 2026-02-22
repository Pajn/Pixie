//! Pixie - macOS Window Focusing Tool

mod accessibility;
mod config;
mod error;
mod event_tap;
mod leader_mode;
mod menu_bar;
mod notification;
mod ui;
mod window;

use clap::{Parser, Subcommand};
use cocoa::appkit::{NSApplication, NSApplicationActivationPolicy};
use cocoa::base::nil;
use gpui::AssetSource;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

struct EmptyAssets;
impl AssetSource for EmptyAssets {
    fn load(&self, _path: &str) -> anyhow::Result<Option<std::borrow::Cow<'static, [u8]>>> {
        Ok(None)
    }
    fn list(&self, _path: &str) -> anyhow::Result<Vec<gpui::SharedString>> {
        Ok(Vec::new())
    }
}

use config::Action;
use error::{PixieError, Result};
use event_tap::EventTapAction;
use leader_mode::{LeaderModeController, LeaderModeEvent};
use window::WindowManager;

struct WindowManagerState(pub Arc<WindowManager>);
impl gpui::Global for WindowManagerState {}

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

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let is_from_terminal = std::env::var("TERM_PROGRAM").is_ok();
    if is_from_terminal {
        println!("Note: Running from Terminal. If permissions don't work,");
        println!("      try running as: open /Applications/Pixie.app\n");
    }

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

fn handle_keybind_action(action: &Action, _window_manager: &WindowManager) {
    match action {
        Action::FocusLeft | Action::FocusRight | Action::FocusUp | Action::FocusDown => {
            let direction = match action {
                Action::FocusLeft => accessibility::Direction::Left,
                Action::FocusRight => accessibility::Direction::Right,
                Action::FocusUp => accessibility::Direction::Up,
                Action::FocusDown => accessibility::Direction::Down,
                _ => unreachable!(),
            };

            match accessibility::get_focused_window() {
                Ok(focused_element) => match accessibility::get_window_rect(&focused_element) {
                    Ok(from_rect) => {
                        match accessibility::find_window_in_direction(&from_rect, direction) {
                            Ok(target_window) => {
                                if let Err(e) = accessibility::focus_window(&target_window) {
                                    eprintln!("✗ Failed to focus window: {}", e);
                                }
                            }
                            Err(e) => eprintln!("✗ No window found {:?}: {}", direction, e),
                        }
                    }
                    Err(e) => eprintln!("✗ Failed to get window rect: {}", e),
                },
                Err(e) => eprintln!("✗ Failed to get focused window: {}", e),
            }
        }
        Action::Minimize => match accessibility::get_focused_window() {
            Ok(element) => {
                if let Err(e) = accessibility::minimize_window(&element) {
                    eprintln!("✗ Failed to minimize window: {}", e);
                }
            }
            Err(e) => eprintln!("✗ Failed to get focused window: {}", e),
        },
        Action::Maximize => match accessibility::get_focused_window() {
            Ok(element) => {
                if let Err(e) = accessibility::maximize_window(&element) {
                    eprintln!("✗ Failed to maximize window: {}", e);
                }
            }
            Err(e) => eprintln!("✗ Failed to get focused window: {}", e),
        },
        Action::Fullscreen => match accessibility::get_focused_window() {
            Ok(element) => {
                if let Err(e) = accessibility::toggle_fullscreen(&element) {
                    eprintln!("✗ Failed to toggle fullscreen: {}", e);
                }
            }
            Err(e) => eprintln!("✗ Failed to get focused window: {}", e),
        },
        Action::MoveMonitorLeft
        | Action::MoveMonitorRight
        | Action::MoveMonitorUp
        | Action::MoveMonitorDown => {
            let direction = match action {
                Action::MoveMonitorLeft => accessibility::MonitorDirection::Left,
                Action::MoveMonitorRight => accessibility::MonitorDirection::Right,
                Action::MoveMonitorUp => accessibility::MonitorDirection::Up,
                Action::MoveMonitorDown => accessibility::MonitorDirection::Down,
                _ => unreachable!(),
            };

            match accessibility::get_focused_window() {
                Ok(element) => {
                    if let Err(e) = accessibility::move_window_to_monitor(&element, direction) {
                        eprintln!("✗ Failed to move window to monitor: {}", e);
                    }
                }
                Err(e) => eprintln!("✗ Failed to get focused window: {}", e),
            }
        }
        Action::Center => match accessibility::get_focused_window() {
            Ok(element) => {
                let placements = config::builtin_placements();
                if let Some(placement) = placements.get("center")
                    && let Err(e) = accessibility::apply_placement(&element, placement)
                {
                    eprintln!("✗ Failed to center window: {}", e);
                }
            }
            Err(e) => eprintln!("✗ Failed to get focused window: {}", e),
        },
        Action::Place(name) => match accessibility::get_focused_window() {
            Ok(element) => {
                let config = config::load().unwrap_or_else(|e| {
                    eprintln!("Error loading config: {}", e);
                    eprintln!("Please fix your config file or remove it to use defaults.");
                    std::process::exit(1);
                });
                let placements = config.get_placements();
                if let Some(placement) = placements.get(name) {
                    if let Err(e) = accessibility::apply_placement(&element, placement) {
                        eprintln!("✗ Failed to apply placement '{}': {}", name, e);
                    }
                } else {
                    eprintln!("✗ Placement '{}' not found", name);
                }
            }
            Err(e) => eprintln!("✗ Failed to get focused window: {}", e),
        },
        Action::Tile => {}
    }
}

fn apply_autostart_setting(enabled: bool) {
    if enabled && !config::is_autostart_enabled() {
        if let Err(e) = config::set_autostart(true) {
            eprintln!("Warning: Failed to enable autostart: {}", e);
        }
    } else if !enabled
        && config::is_autostart_enabled()
        && let Err(e) = config::set_autostart(false)
    {
        eprintln!("Warning: Failed to disable autostart: {}", e);
    }
}

fn runtime_bindings(cfg: &config::Config) -> (
    config::Modifiers,
    config::KeyCode,
    Vec<config::KeybindEntry>,
    Duration,
) {
    let (leader_modifiers, leader_keycode) = config::parse_leader_key(&cfg.leader_key)
        .unwrap_or_else(|e| {
            eprintln!(
                "Warning: Invalid leader key '{}': {}. Falling back to cmd+shift+a.",
                cfg.leader_key, e
            );
            (
                Some(config::Modifiers::SUPER | config::Modifiers::SHIFT),
                config::KeyCode::KeyA,
            )
        });
    let keybinds = cfg.parsed_keybinds();
    if keybinds.len() != cfg.keybinds.len() {
        eprintln!("Warning: Some keybinds are invalid and were ignored.");
    }

    (
        leader_modifiers.unwrap_or(config::Modifiers::SUPER | config::Modifiers::SHIFT),
        leader_keycode,
        keybinds,
        Duration::from_secs(cfg.timeout),
    )
}

fn run_daemon(window_manager: Arc<WindowManager>, headless: bool) -> Result<()> {
    let config = config::load().unwrap_or_else(|e| {
        eprintln!("Error loading config: {}", e);
        eprintln!("Please fix your config file or remove it to use defaults.");
        std::process::exit(1);
    });
    apply_autostart_setting(config.autostart);
    let (leader_modifiers, leader_keycode, keybinds, leader_timeout) = runtime_bindings(&config);
    let leader_keybinds: Vec<_> = keybinds
        .iter()
        .filter(|k| matches!(k.keybind, config::Keybind::LeaderPrefixed { .. }))
        .collect();

    println!("🧚 Pixie started");
    println!(
        "  {} - Leader key (then press a letter to focus, or Shift+letter to register)",
        config.leader_key
    );

    if !leader_keybinds.is_empty() {
        println!("  Leader-prefixed keybinds:");
        for entry in leader_keybinds {
            println!("    {:?} -> {:?}", entry.keybind, entry.action);
        }
    }

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

    if headless {
        println!("Running in headless mode (Ctrl+C to quit)...");
        run_headless_only(
            window_manager,
            leader_modifiers,
            leader_keycode,
            keybinds,
            leader_timeout,
        )?;
        return Ok(());
    }

    enum UiAction {
        ShowWindowPicker,
        PickerInput(ui::PickerInput),
        MenuBarRefresh,
        MenuBarSetActive(bool),
        Quit,
    }

    let (ui_sender, mut ui_receiver) = tokio::sync::mpsc::unbounded_channel::<UiAction>();
    let (event_sender, mut event_receiver) =
        tokio::sync::mpsc::unbounded_channel::<EventTapAction>();

    let wm_for_events = Arc::clone(&window_manager);
    let menubar_enabled = config.menubar_icon;
    let menubar_active_color = config.menubar_active_color.clone();

    gpui::Application::new()
        .with_assets(EmptyAssets)
        .run(move |cx: &mut gpui::App| {
            unsafe {
                let ns_app = NSApplication::sharedApplication(nil);
                ns_app.setActivationPolicy_(
                    NSApplicationActivationPolicy::NSApplicationActivationPolicyRegular,
                );
                ns_app.activateIgnoringOtherApps_(true);
            }

            tracing::trace!(
                "creating event tap with leader_modifiers={:?}, leader_keycode={:?}",
                leader_modifiers,
                leader_keycode
            );
            let event_tap = event_tap::EventTap::new(
                leader_modifiers,
                leader_keycode,
                keybinds.clone(),
                event_sender.clone(),
            );

            if let Err(e) = &event_tap {
                eprintln!("\n❌ Failed to create event tap:\n{}\n", e);
                eprintln!("Pixie needs Accessibility permissions to monitor keyboard events.");
                eprintln!("Please grant permissions and restart Pixie.");
                let _ = ui_sender.send(UiAction::Quit);
                return;
            }
            let mut event_tap = event_tap.unwrap();

            let leader_mode_controller = Arc::new(
                LeaderModeController::with_timeout(leader_timeout)
                    .expect("Failed to create leader mode controller"),
            );

            ui::init(cx);

            cx.set_global(WindowManagerState(wm_for_events.clone()));

            let menu_bar_controller = if menubar_enabled {
                match menu_bar::MenuBarController::new(
                    wm_for_events.clone(),
                    menubar_active_color.clone(),
                ) {
                    Ok(controller) => Some(controller),
                    Err(e) => {
                        eprintln!("Warning: Failed to create menu bar icon: {}", e);
                        None
                    }
                }
            } else {
                None
            };

            let leader_event_receiver = leader_mode_controller.events();
            let controller = Arc::clone(&leader_mode_controller);
            let wm = Arc::clone(&wm_for_events);
            let ui_sender = ui_sender.clone();
            let event_sender = event_sender.clone();
            let mut watched_menubar_icon = menubar_enabled;
            let mut watched_menubar_active_color = menubar_active_color.clone();

            std::thread::spawn(move || {
                tracing::trace!("event tap thread started");
                let mut config_watcher = config::ConfigWatcher::new();
                let mut last_config_poll = Instant::now();

                loop {
                    if !RUNNING.load(Ordering::SeqCst) {
                        let _ = ui_sender.send(UiAction::Quit);
                        break;
                    }

                    match event_receiver.try_recv() {
                        Ok(event) => {
                            tracing::trace!("received event tap event: {:?}", event);
                            match event {
                                EventTapAction::LeaderPressed => {
                                    controller.enter_listening_mode();
                                    notification::notify("Pixie", "Listening...");
                                    let _ = ui_sender.send(UiAction::MenuBarSetActive(true));
                                }
                                EventTapAction::LeaderReleased => {}
                                EventTapAction::KeyPressed(keycode, has_shift) => {
                                    if let Some(letter) = keycode_to_letter(keycode) {
                                        controller.handle_key(letter, has_shift);
                                    }
                                }
                                EventTapAction::ActionTriggered(action) => {
                                    controller.handle_action(action);
                                }
                                EventTapAction::ArrowPressed(direction) => {
                                    controller.handle_direction(direction);
                                }
                                EventTapAction::PickerInput(input) => {
                                    let _ = ui_sender.send(UiAction::PickerInput(input));
                                }
                            }
                        }
                        Err(tokio::sync::mpsc::error::TryRecvError::Empty) => {}
                        Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {
                            tracing::warn!("event tap receiver disconnected");
                            let _ = ui_sender.send(UiAction::Quit);
                            break;
                        }
                    }

                    if let Ok(event) = leader_event_receiver.try_recv() {
                        let mut refresh_menu = false;
                        match event {
                            LeaderModeEvent::RegisterSlot(c) => {
                                let slot = c.to_ascii_lowercase();
                                match wm.register_current_window(slot) {
                                    Ok((_, window)) => {
                                        notification::notify(
                                            "Pixie",
                                            &format!(
                                                "Registered to [{}]: {}",
                                                slot, window.app_name
                                            ),
                                        );
                                    }
                                    Err(e) => eprintln!("✗ Failed: {}", e),
                                }
                                refresh_menu = true;
                            }
                            LeaderModeEvent::FocusSlot(c) => match wm.focus_saved_window(c) {
                                Ok(window) => {
                                    notification::notify(
                                        "Pixie",
                                        &format!("Focused [{}]: {}", c, window.app_name),
                                    );
                                    refresh_menu = true;
                                }
                                Err(e) => eprintln!("✗ Failed: {}", e),
                            },
                            LeaderModeEvent::Cancelled => {
                                notification::notify("Pixie", "Cancelled");
                            }
                            LeaderModeEvent::KeybindAction(action) => {
                                if matches!(action, Action::Tile) {
                                    let _ = ui_sender.send(UiAction::ShowWindowPicker);
                                } else {
                                    handle_keybind_action(&action, &wm);
                                }
                            }
                            LeaderModeEvent::FocusDirection(direction) => {
                                match accessibility::get_focused_window() {
                                    Ok(focused_element) => {
                                        match accessibility::get_window_rect(&focused_element) {
                                            Ok(from_rect) => {
                                                match accessibility::find_window_in_direction(
                                                    &from_rect, direction,
                                                ) {
                                                    Ok(target_window) => {
                                                        if let Err(e) = accessibility::focus_window(
                                                            &target_window,
                                                        ) {
                                                            eprintln!(
                                                                "✗ Failed to focus window: {}",
                                                                e
                                                            );
                                                        }
                                                    }
                                                    Err(e) => {
                                                        eprintln!(
                                                            "✗ No window found {:?}: {}",
                                                            direction, e
                                                        )
                                                    }
                                                }
                                            }
                                            Err(e) => {
                                                eprintln!("✗ Failed to get window rect: {}", e)
                                            }
                                        }
                                    }
                                    Err(e) => eprintln!("✗ Failed to get focused window: {}", e),
                                }
                            }
                        }
                        let _ = ui_sender.send(UiAction::MenuBarSetActive(false));
                        if refresh_menu {
                            let _ = ui_sender.send(UiAction::MenuBarRefresh);
                        }
                    }

                    if last_config_poll.elapsed() >= Duration::from_millis(500) {
                        last_config_poll = Instant::now();
                        if let Some(reload) = config_watcher.poll_changed() {
                            match reload {
                                Ok(new_config) => {
                                    let (
                                        new_leader_modifiers,
                                        new_leader_keycode,
                                        new_keybinds,
                                        new_timeout,
                                    ) = runtime_bindings(&new_config);
                                    match event_tap::EventTap::new(
                                        new_leader_modifiers,
                                        new_leader_keycode,
                                        new_keybinds,
                                        event_sender.clone(),
                                    ) {
                                        Ok(new_event_tap) => {
                                            event_tap = new_event_tap;
                                            controller.set_timeout(new_timeout);
                                            apply_autostart_setting(new_config.autostart);
                                            if new_config.menubar_icon != watched_menubar_icon
                                                || new_config.menubar_active_color
                                                    != watched_menubar_active_color
                                            {
                                                eprintln!(
                                                    "Config updated: menubar changes apply after restart."
                                                );
                                            }
                                            watched_menubar_icon = new_config.menubar_icon;
                                            watched_menubar_active_color =
                                                new_config.menubar_active_color.clone();
                                            println!("↻ Reloaded config");
                                            notification::notify("Pixie", "Config reloaded");
                                        }
                                        Err(e) => {
                                            eprintln!(
                                                "Warning: Config changed but hotkeys were not reloaded: {}",
                                                e
                                            );
                                        }
                                    }
                                }
                                Err(e) => {
                                    eprintln!("Warning: Failed to reload config: {}", e);
                                }
                            }
                        }
                    }

                    let _ = &event_tap;
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
            });

            cx.spawn(|cx| async move {
                while let Some(action) = ui_receiver.recv().await {
                    match action {
                        UiAction::ShowWindowPicker => {
                            cx.update(|cx| {
                                unsafe {
                                    let ns_app = NSApplication::sharedApplication(nil);
                                    ns_app.activateIgnoringOtherApps_(true);
                                }
                                ui::show_window_picker(cx);
                            })
                            .ok();
                        }
                        UiAction::PickerInput(input) => {
                            cx.update(|cx| {
                                ui::handle_picker_input(input, cx);
                            })
                            .ok();
                        }
                        UiAction::MenuBarRefresh => {
                            cx.update(|_| {
                                if let Some(controller) = menu_bar_controller.as_ref() {
                                    controller.refresh_menu();
                                }
                            })
                            .ok();
                        }
                        UiAction::MenuBarSetActive(active) => {
                            cx.update(|_| {
                                if let Some(controller) = menu_bar_controller.as_ref() {
                                    controller.set_leader_mode_active(active);
                                }
                            })
                            .ok();
                        }
                        UiAction::Quit => {
                            cx.update(|cx| cx.quit()).ok();
                            break;
                        }
                    }
                }
            })
            .detach();
        });

    Ok(())
}

fn keycode_to_letter(keycode: i64) -> Option<char> {
    match keycode {
        0 => Some('a'),
        1 => Some('s'),
        2 => Some('d'),
        3 => Some('f'),
        4 => Some('h'),
        5 => Some('g'),
        6 => Some('z'),
        7 => Some('x'),
        8 => Some('c'),
        9 => Some('v'),
        11 => Some('b'),
        12 => Some('q'),
        13 => Some('w'),
        14 => Some('e'),
        15 => Some('r'),
        16 => Some('y'),
        17 => Some('t'),
        31 => Some('o'),
        32 => Some('u'),
        34 => Some('i'),
        35 => Some('p'),
        38 => Some('j'),
        40 => Some('k'),
        37 => Some('l'),
        46 => Some('m'),
        45 => Some('n'),
        _ => None,
    }
}

fn run_headless_only(
    window_manager: Arc<WindowManager>,
    leader_modifiers: config::Modifiers,
    leader_keycode: config::KeyCode,
    keybinds: Vec<config::KeybindEntry>,
    leader_timeout: Duration,
) -> Result<()> {
    let leader_mode_controller = Arc::new(LeaderModeController::with_timeout(leader_timeout)?);

    let (event_sender, mut event_receiver) =
        tokio::sync::mpsc::unbounded_channel::<EventTapAction>();
    tracing::trace!(
        "creating headless event tap with leader_modifiers={:?}, leader_keycode={:?}",
        leader_modifiers,
        leader_keycode
    );
    let event_tap = event_tap::EventTap::new(
        leader_modifiers,
        leader_keycode,
        keybinds.clone(),
        event_sender.clone(),
    );

    if let Err(e) = &event_tap {
        eprintln!("\n❌ Failed to create event tap:\n{}\n", e);
        eprintln!("Pixie needs Accessibility permissions to monitor keyboard events.");
        eprintln!("Please grant permissions and restart Pixie.");
        return Err(PixieError::EventTap(e.clone()));
    }
    let mut event_tap = event_tap.unwrap();

    let controller_for_event = Arc::clone(&leader_mode_controller);
    let wm_for_events = Arc::clone(&window_manager);
    let leader_event_receiver = leader_mode_controller.events();
    let event_sender = event_sender.clone();

    std::thread::spawn(move || {
        let mut config_watcher = config::ConfigWatcher::new();
        let mut last_config_poll = Instant::now();
        loop {
            if !RUNNING.load(Ordering::SeqCst) {
                break;
            }

            match event_receiver.try_recv() {
                Ok(event) => match event {
                    EventTapAction::LeaderPressed => {
                        controller_for_event.enter_listening_mode();
                        notification::notify("Pixie", "Listening...");
                        println!("Listening...");
                    }
                    EventTapAction::LeaderReleased => {}
                    EventTapAction::KeyPressed(keycode, has_shift) => {
                        if let Some(letter) = keycode_to_letter(keycode) {
                            controller_for_event.handle_key(letter, has_shift);
                        }
                    }
                    EventTapAction::ActionTriggered(action) => {
                        controller_for_event.handle_action(action);
                    }
                    EventTapAction::ArrowPressed(direction) => {
                        controller_for_event.handle_direction(direction);
                    }
                    EventTapAction::PickerInput(_) => {}
                },
                Err(tokio::sync::mpsc::error::TryRecvError::Empty) => {}
                Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {
                    break;
                }
            }

            if let Ok(event) = leader_event_receiver.try_recv() {
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
                    LeaderModeEvent::KeybindAction(action) => {
                        handle_keybind_action(&action, &wm_for_events);
                    }
                    LeaderModeEvent::FocusDirection(direction) => {
                        match accessibility::get_focused_window() {
                            Ok(focused_element) => {
                                match accessibility::get_window_rect(&focused_element) {
                                    Ok(from_rect) => match accessibility::find_window_in_direction(
                                        &from_rect, direction,
                                    ) {
                                        Ok(target_window) => {
                                            if let Err(e) =
                                                accessibility::focus_window(&target_window)
                                            {
                                                eprintln!("✗ Failed to focus window: {}", e);
                                            }
                                        }
                                        Err(e) => {
                                            eprintln!("✗ No window found {:?}: {}", direction, e)
                                        }
                                    },
                                    Err(e) => eprintln!("✗ Failed to get window rect: {}", e),
                                }
                            }
                            Err(e) => eprintln!("✗ Failed to get focused window: {}", e),
                        }
                    }
                }
            }

            if last_config_poll.elapsed() >= Duration::from_millis(500) {
                last_config_poll = Instant::now();
                if let Some(reload) = config_watcher.poll_changed() {
                    match reload {
                        Ok(new_config) => {
                            let (
                                new_leader_modifiers,
                                new_leader_keycode,
                                new_keybinds,
                                new_timeout,
                            ) = runtime_bindings(&new_config);
                            match event_tap::EventTap::new(
                                new_leader_modifiers,
                                new_leader_keycode,
                                new_keybinds,
                                event_sender.clone(),
                            ) {
                                Ok(new_event_tap) => {
                                    event_tap = new_event_tap;
                                    controller_for_event.set_timeout(new_timeout);
                                    apply_autostart_setting(new_config.autostart);
                                    println!("↻ Reloaded config");
                                }
                                Err(e) => {
                                    eprintln!(
                                        "Warning: Config changed but hotkeys were not reloaded: {}",
                                        e
                                    );
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("Warning: Failed to reload config: {}", e);
                        }
                    }
                }
            }

            let _ = &event_tap;
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    });

    while RUNNING.load(Ordering::SeqCst) {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    Ok(())
}

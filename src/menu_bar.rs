//! Menu bar UI for Pixie

#![allow(unexpected_cfgs)]

use cocoa::appkit::{
    NSApp, NSButton, NSColor, NSCompositingOperation, NSImage, NSMenu, NSMenuItem, NSRectFill,
    NSStatusBar, NSStatusItem, NSVariableStatusItemLength,
};
use cocoa::base::{NO, YES, id, nil};
use cocoa::foundation::{NSPoint, NSRect, NSSize, NSString};
use objc::declare::ClassDecl;
use objc::runtime::{Class, Object, Sel};
use objc::{class, msg_send, sel, sel_impl};
use std::fs::OpenOptions;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, OnceLock};

use crate::config;
use crate::error::{PixieError, Result};
use crate::window::WindowManager;

/// Menu bar controller
pub struct MenuBarController {
    window_manager: Arc<WindowManager>,
    status_item: id,
    icon_image: id,
    active_icon_image: id,
    menu_target: id,
}

impl MenuBarController {
    /// Create a new menu bar controller
    pub fn new(
        window_manager: Arc<WindowManager>,
        active_color_hex: Option<String>,
    ) -> Result<Self> {
        unsafe {
            let status_bar = NSStatusBar::systemStatusBar(nil);
            let status_item = status_bar.statusItemWithLength_(NSVariableStatusItemLength);
            if status_item == nil {
                return Err(PixieError::MenuBar(
                    "Failed to create status bar item".to_string(),
                ));
            }
            let _: id = msg_send![status_item, retain];
            let active_color = active_color_hex
                .as_deref()
                .and_then(ns_color_from_hex)
                .unwrap_or_else(default_active_color);
            let (icon_image, active_icon_image) = load_status_images(active_color);
            let menu_target = create_menu_target();

            let controller = MenuBarController {
                window_manager,
                status_item,
                icon_image,
                active_icon_image,
                menu_target,
            };
            controller.configure_button_icon();
            controller.refresh_menu();
            controller.set_leader_mode_active(false);
            Ok(controller)
        }
    }

    pub fn set_leader_mode_active(&self, active: bool) {
        unsafe {
            let button = self.status_item.button();
            if button == nil {
                return;
            }

            let _: () = msg_send![button, setContentTintColor: nil];
            if self.icon_image != nil {
                let image = if active && self.active_icon_image != nil {
                    self.active_icon_image
                } else {
                    self.icon_image
                };
                button.setImage_(image);
            } else {
                let fallback_title = NSString::alloc(nil).init_str("🧚");
                let _: () = msg_send![button, setTitle: fallback_title];
            }
        }
    }

    pub fn refresh_menu(&self) {
        unsafe {
            let menu = NSMenu::new(nil);
            menu.setAutoenablesItems(NO);

            let saved_windows = self.window_manager.get_all_saved_windows();
            if saved_windows.is_empty() {
                self.add_disabled_menu_item(menu, "No windows registered");
            } else {
                self.add_disabled_menu_item(menu, "Saved windows");

                let mut windows: Vec<_> = saved_windows.into_iter().collect();
                windows.sort_by_key(|(slot, _)| *slot);

                for (slot, window) in windows {
                    self.add_disabled_menu_item(
                        menu,
                        &format!("[{}] {}", slot, window.display_string()),
                    );
                }
            }

            menu.addItem_(NSMenuItem::separatorItem(nil));
            self.add_open_config_menu_item(menu);

            menu.addItem_(NSMenuItem::separatorItem(nil));
            let quit_title = NSString::alloc(nil).init_str("Quit Pixie");
            let quit_key = NSString::alloc(nil).init_str("q");
            let quit_item =
                menu.addItemWithTitle_action_keyEquivalent(quit_title, sel!(terminate:), quit_key);
            NSMenuItem::setTarget_(quit_item, NSApp());
            self.status_item.setMenu_(menu);
        }
    }

    fn configure_button_icon(&self) {
        unsafe {
            let button = self.status_item.button();
            if button == nil {
                return;
            }

            if self.icon_image != nil {
                button.setImage_(self.icon_image);
                return;
            }

            let fallback_title = NSString::alloc(nil).init_str("🧚");
            let _: () = msg_send![button, setTitle: fallback_title];
        }
    }

    fn add_disabled_menu_item(&self, menu: id, title: &str) {
        unsafe {
            let ns_title = NSString::alloc(nil).init_str(title);
            let ns_empty = NSString::alloc(nil).init_str("");
            let item = menu.addItemWithTitle_action_keyEquivalent(ns_title, sel!(null), ns_empty);
            let _: () = msg_send![item, setEnabled: NO];
        }
    }

    fn add_open_config_menu_item(&self, menu: id) {
        unsafe {
            let title = NSString::alloc(nil).init_str("Open Config");
            let key = NSString::alloc(nil).init_str(",");
            let item = menu.addItemWithTitle_action_keyEquivalent(title, sel!(openConfig:), key);
            NSMenuItem::setTarget_(item, self.menu_target);
        }
    }
}

impl Drop for MenuBarController {
    fn drop(&mut self) {
        unsafe {
            let status_bar = NSStatusBar::systemStatusBar(nil);
            status_bar.removeStatusItem_(self.status_item);
            if self.icon_image != nil {
                let _: () = msg_send![self.icon_image, release];
            }
            if self.active_icon_image != nil {
                let _: () = msg_send![self.active_icon_image, release];
            }
            if self.menu_target != nil {
                let _: () = msg_send![self.menu_target, release];
            }
            let _: () = msg_send![self.status_item, release];
        }
    }
}

fn create_menu_target() -> id {
    static TARGET_CLASS: OnceLock<usize> = OnceLock::new();

    unsafe {
        let class_ptr = *TARGET_CLASS.get_or_init(|| {
            if let Some(existing) = Class::get("PixieMenuTarget") {
                return existing as *const Class as usize;
            }

            let mut decl = ClassDecl::new("PixieMenuTarget", class!(NSObject))
                .expect("failed to declare PixieMenuTarget");
            decl.add_method(
                sel!(openConfig:),
                open_config_action as extern "C" fn(&Object, Sel, id),
            );
            decl.register() as *const Class as usize
        }) as *const Class;

        let target: id = msg_send![class_ptr, new];
        target
    }
}

extern "C" fn open_config_action(_: &Object, _: Sel, _: id) {
    if let Err(e) = open_config_in_editor() {
        eprintln!("Failed to open config: {}", e);
    }
}

fn open_config_in_editor() -> Result<()> {
    let path = config::config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            PixieError::Config(format!(
                "Failed to create config directory {:?}: {}",
                parent, e
            ))
        })?;
    }
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| {
            PixieError::Config(format!("Failed to create config file {:?}: {}", path, e))
        })?;

    let status = Command::new("open")
        .arg("-t")
        .arg(&path)
        .status()
        .map_err(|e| PixieError::Config(format!("Failed to run open command: {}", e)))?;
    if !status.success() {
        return Err(PixieError::Config(format!(
            "open command failed with status: {}",
            status
        )));
    }

    Ok(())
}

fn load_status_images(active_color: id) -> (id, id) {
    unsafe {
        if let Some(path) = resolve_icon_path()
            && let Some(path_str) = path.to_str()
        {
            let ns_path = NSString::alloc(nil).init_str(path_str);
            let image = NSImage::alloc(nil).initWithContentsOfFile_(ns_path);
            if image != nil && image.isValid() == YES {
                let _: () = msg_send![image, setSize: NSSize::new(18.0, 18.0)];
                let _: () = msg_send![image, setTemplate: YES];
                let _: id = msg_send![image, retain];
                let active_image = build_active_icon_image(image, active_color);
                return (image, active_image);
            }
        }
    }

    (nil, nil)
}

fn build_active_icon_image(source_image: id, active_color: id) -> id {
    unsafe {
        let size = source_image.size();
        let source_copy: id = msg_send![source_image, copy];
        if source_copy == nil {
            return nil;
        }
        let _: () = msg_send![source_copy, setTemplate: NO];

        let active_image = NSImage::alloc(nil).initWithSize_(size);
        if active_image == nil {
            let _: () = msg_send![source_copy, release];
            return nil;
        }

        active_image.lockFocus();
        let rect = NSRect::new(NSPoint::new(0.0, 0.0), size);
        let _: () = msg_send![active_color, set];
        NSRectFill(rect);
        source_copy.drawInRect_fromRect_operation_fraction_(
            rect,
            rect,
            NSCompositingOperation::NSCompositeDestinationIn,
            1.0,
        );
        active_image.unlockFocus();
        let _: () = msg_send![active_image, setTemplate: NO];
        let _: id = msg_send![active_image, retain];
        let _: () = msg_send![source_copy, release];
        active_image
    }
}

fn resolve_icon_path() -> Option<PathBuf> {
    let mut candidates = Vec::new();

    if let Ok(exe_path) = std::env::current_exe()
        && let Some(macos_dir) = exe_path.parent()
        && let Some(contents_dir) = macos_dir.parent()
    {
        candidates.push(contents_dir.join("Resources").join("menuTemplate@2x.png"));
    }

    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("menuTemplate@2x.png"));
    }

    candidates.into_iter().find(|path| path.exists())
}

fn parse_hex_color(input: &str) -> Option<(f64, f64, f64)> {
    let hex = input.trim().trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }

    let rgb = u32::from_str_radix(hex, 16).ok()?;
    let r = ((rgb >> 16) & 0xff) as f64 / 255.0;
    let g = ((rgb >> 8) & 0xff) as f64 / 255.0;
    let b = (rgb & 0xff) as f64 / 255.0;
    Some((r, g, b))
}

fn ns_color_from_hex(input: &str) -> Option<id> {
    let (r, g, b) = parse_hex_color(input)?;
    unsafe {
        Some(NSColor::colorWithSRGBRed_green_blue_alpha_(
            nil, r, g, b, 1.0,
        ))
    }
}

fn default_active_color() -> id {
    unsafe {
        let supports_accent: cocoa::base::BOOL =
            msg_send![class!(NSColor), respondsToSelector: sel!(controlAccentColor)];
        if supports_accent == YES {
            msg_send![class!(NSColor), controlAccentColor]
        } else {
            let accent_code = Command::new("defaults")
                .args(["read", "-g", "AppleAccentColor"])
                .output()
                .ok()
                .and_then(|output| {
                    if output.status.success() {
                        String::from_utf8(output.stdout)
                            .ok()
                            .and_then(|value| value.trim().parse::<i32>().ok())
                    } else {
                        None
                    }
                })
                .unwrap_or(4);

            let (r, g, b) = match accent_code {
                -1 => (0.56, 0.56, 0.58),
                0 => (1.00, 0.23, 0.19),
                1 => (1.00, 0.58, 0.00),
                2 => (1.00, 0.80, 0.00),
                3 => (0.20, 0.78, 0.35),
                5 => (0.69, 0.32, 0.87),
                6 => (1.00, 0.18, 0.33),
                _ => (0.00, 0.48, 1.00),
            };

            NSColor::colorWithSRGBRed_green_blue_alpha_(nil, r, g, b, 1.0)
        }
    }
}

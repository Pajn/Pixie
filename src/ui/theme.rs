use std::sync::OnceLock;

use cocoa::base::{YES, id, nil};
use gpui::{rgb, rgba};
use objc::{class, msg_send, sel, sel_impl};

#[derive(Clone, Copy, Debug)]
pub struct Theme {
    pub background: gpui::Rgba,
    pub foreground: gpui::Rgba,
    pub selected: gpui::Rgba,
    pub muted: gpui::Rgba,
    pub muted_foreground: gpui::Rgba,
    pub border: gpui::Rgba,
    pub accent: gpui::Rgba,
}

impl Default for Theme {
    fn default() -> Self {
        let accent =
            rgb(*SYSTEM_ACCENT_HEX.get_or_init(|| system_accent_hex().unwrap_or(0x60a5fa)));
        Self {
            background: rgba(0x11131866),
            foreground: rgb(0xffffff),
            selected: rgba(0xffffff30),
            muted: rgba(0xffffff1f),
            muted_foreground: rgb(0xa0a8b4),
            border: rgb(0x2f3540),
            accent,
        }
    }
}

static SYSTEM_ACCENT_HEX: OnceLock<u32> = OnceLock::new();

fn system_accent_hex() -> Option<u32> {
    unsafe {
        let supports_accent: cocoa::base::BOOL =
            msg_send![class!(NSColor), respondsToSelector: sel!(controlAccentColor)];
        if supports_accent != YES {
            return None;
        }

        let accent_color: id = msg_send![class!(NSColor), controlAccentColor];
        if accent_color == nil {
            return None;
        }

        let color_space: id = msg_send![class!(NSColorSpace), sRGBColorSpace];
        if color_space == nil {
            return None;
        }

        let srgb_color: id = msg_send![accent_color, colorUsingColorSpace: color_space];
        if srgb_color == nil {
            return None;
        }

        let red: f64 = msg_send![srgb_color, redComponent];
        let green: f64 = msg_send![srgb_color, greenComponent];
        let blue: f64 = msg_send![srgb_color, blueComponent];

        Some(
            ((to_channel(red) as u32) << 16)
                | ((to_channel(green) as u32) << 8)
                | (to_channel(blue) as u32),
        )
    }
}

fn to_channel(value: f64) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}

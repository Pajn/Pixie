use gpui::rgb;

#[derive(Clone, Copy, Debug)]
pub struct Theme {
    pub background: gpui::Rgba,
    pub foreground: gpui::Rgba,
    pub muted: gpui::Rgba,
    pub muted_foreground: gpui::Rgba,
    pub border: gpui::Rgba,
    pub primary: gpui::Rgba,
    pub accent: gpui::Rgba,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            background: rgb(0x1e1e1e),
            foreground: rgb(0xffffff),
            muted: rgb(0x3c3c3c),
            muted_foreground: rgb(0x9ca3af),
            border: rgb(0x3c3c3c),
            primary: rgb(0x3b82f6),
            accent: rgb(0x3b82f6),
        }
    }
}

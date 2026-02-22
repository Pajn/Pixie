use gpui::{
    AnyElement, App, ClickEvent, Div, ElementId, InteractiveElement, IntoElement, MouseButton,
    MouseMoveEvent, ParentElement, RenderOnce, Stateful, StatefulInteractiveElement, Styled,
    Window, div, px, rgba,
};

use crate::ui::Theme;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum ListItemMode {
    #[default]
    Entry,
    Separator,
}

impl ListItemMode {
    #[inline]
    fn is_separator(&self) -> bool {
        matches!(self, ListItemMode::Separator)
    }
}

#[derive(IntoElement)]
pub struct ListItem {
    base: Stateful<Div>,
    mode: ListItemMode,
    disabled: bool,
    selected: bool,
    secondary_selected: bool,
    confirmed: bool,
    check_icon: Option<AnyElement>,
    on_click: Option<Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>>,
    on_mouse_enter: Option<Box<dyn Fn(&MouseMoveEvent, &mut Window, &mut App) + 'static>>,
    suffix: Option<AnyElement>,
    children: Vec<AnyElement>,
}

impl ListItem {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            base: div().id(id.into()),
            mode: ListItemMode::Entry,
            disabled: false,
            selected: false,
            secondary_selected: false,
            confirmed: false,
            check_icon: None,
            on_click: None,
            on_mouse_enter: None,
            suffix: None,
            children: Vec::new(),
        }
    }

    pub fn separator(mut self) -> Self {
        self.mode = ListItemMode::Separator;
        self
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    pub fn secondary_selected(mut self, selected: bool) -> Self {
        self.secondary_selected = selected;
        self
    }

    pub fn confirmed(mut self, confirmed: bool) -> Self {
        self.confirmed = confirmed;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn check_icon<E>(mut self, icon: E) -> Self
    where
        E: IntoElement,
    {
        self.check_icon = Some(icon.into_any_element());
        self
    }

    pub fn suffix<E>(mut self, element: E) -> Self
    where
        E: IntoElement,
    {
        self.suffix = Some(element.into_any_element());
        self
    }

    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Box::new(handler));
        self
    }

    pub fn on_mouse_enter(
        mut self,
        handler: impl Fn(&MouseMoveEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_mouse_enter = Some(Box::new(handler));
        self
    }
}

impl ParentElement for ListItem {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for ListItem {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let theme = Theme::default();
        let is_selectable = !(self.disabled || self.mode.is_separator());
        let is_active = self.confirmed || self.selected;

        let mut item = self
            .base
            .flex()
            .items_center()
            .justify_between()
            .gap_2()
            .h(px(36.0))
            .w_full()
            .px_3()
            .rounded_md()
            .border_1()
            .border_color(rgba(0x00000000))
            .text_color(theme.foreground);

        if is_selectable {
            item = item.cursor_pointer();
            if let Some(on_click) = self.on_click {
                item = item
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        cx.stop_propagation();
                    })
                    .on_click(on_click);
            }
            if let Some(on_mouse_enter) = self.on_mouse_enter {
                item = item.on_mouse_move(move |ev, window, cx| (on_mouse_enter)(ev, window, cx));
            }
            if !is_active && !self.secondary_selected {
                item = item.hover(|hovered| hovered.bg(theme.muted));
            }
        } else {
            item = item.text_color(theme.muted_foreground);
        }

        if is_active {
            item = item.bg(theme.selected);
        } else if self.secondary_selected {
            item = item.bg(theme.muted);
        }
        if self.secondary_selected {
            item = item.border_color(theme.accent);
        }

        let mut content = div()
            .flex()
            .w_full()
            .items_center()
            .gap_2()
            .child(div().flex_1().overflow_hidden().children(self.children));

        if let Some(suffix) = self.suffix {
            content = content.child(div().flex_none().child(suffix));
        }

        if self.confirmed
            && let Some(icon) = self.check_icon
        {
            content = content.child(icon);
        }

        item = item.child(content);

        item
    }
}

use std::ops::Range;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

use gpui::{
    App, Bounds, Context, Entity, FocusHandle, Focusable, Global, InteractiveElement, IntoElement,
    KeyBinding, ParentElement, Render, Size, UniformListScrollHandle, Window,
    WindowBackgroundAppearance, WindowBounds, WindowHandle, WindowKind, WindowOptions, actions,
    div, img, prelude::*, px, uniform_list,
};

use crate::accessibility::{
    WindowEntry, find_window_by_id, focus_window, get_all_windows, get_focused_window,
    get_screen_for_window, get_screens, get_window_rect, tile_windows_in_columns,
};
use crate::ui::{ListItem, Theme};

actions!(
    window_picker,
    [SelectDown, SelectUp, ToggleSelect, Confirm, Cancel]
);

static WINDOW_PICKER_ACTIVE: AtomicBool = AtomicBool::new(false);
const WINDOW_PICKER_KEY_CONTEXT: &str = "WindowPicker";
const PICKER_WIDTH: f32 = 560.0;
const PICKER_KEY_INPUTS: [(&str, PickerInput); 8] = [
    ("j", PickerInput::SelectDown),
    ("down", PickerInput::SelectDown),
    ("k", PickerInput::SelectUp),
    ("up", PickerInput::SelectUp),
    ("space", PickerInput::ToggleSelect),
    ("enter", PickerInput::Confirm),
    ("q", PickerInput::Cancel),
    ("escape", PickerInput::Cancel),
];

#[derive(Debug, Clone, Copy)]
pub enum PickerInput {
    SelectDown,
    SelectUp,
    ToggleSelect,
    Confirm,
    Cancel,
    SearchBackspace,
    SearchChar(char),
}

pub fn init(cx: &mut App) {
    cx.bind_keys(
        PICKER_KEY_INPUTS
            .iter()
            .map(|(key, input)| picker_key_binding(key, *input)),
    );
}

pub fn is_window_picker_active() -> bool {
    WINDOW_PICKER_ACTIVE.load(Ordering::SeqCst)
}

pub fn picker_input_from_keycode(keycode: i64, shift: bool) -> Option<PickerInput> {
    match keycode {
        125 => Some(PickerInput::SelectDown),
        126 => Some(PickerInput::SelectUp),
        36 => Some(PickerInput::Confirm),
        53 => Some(PickerInput::Cancel),
        51 | 117 => Some(PickerInput::SearchBackspace),
        _ => printable_char_from_keycode(keycode, shift).map(PickerInput::SearchChar),
    }
}

fn printable_char_from_keycode(keycode: i64, shift: bool) -> Option<char> {
    match keycode {
        0 => Some(if shift { 'A' } else { 'a' }),
        1 => Some(if shift { 'S' } else { 's' }),
        2 => Some(if shift { 'D' } else { 'd' }),
        3 => Some(if shift { 'F' } else { 'f' }),
        4 => Some(if shift { 'H' } else { 'h' }),
        5 => Some(if shift { 'G' } else { 'g' }),
        6 => Some(if shift { 'Z' } else { 'z' }),
        7 => Some(if shift { 'X' } else { 'x' }),
        8 => Some(if shift { 'C' } else { 'c' }),
        9 => Some(if shift { 'V' } else { 'v' }),
        11 => Some(if shift { 'B' } else { 'b' }),
        12 => Some(if shift { 'Q' } else { 'q' }),
        13 => Some(if shift { 'W' } else { 'w' }),
        14 => Some(if shift { 'E' } else { 'e' }),
        15 => Some(if shift { 'R' } else { 'r' }),
        16 => Some(if shift { 'Y' } else { 'y' }),
        17 => Some(if shift { 'T' } else { 't' }),
        18 => Some(if shift { '!' } else { '1' }),
        19 => Some(if shift { '@' } else { '2' }),
        20 => Some(if shift { '#' } else { '3' }),
        21 => Some(if shift { '$' } else { '4' }),
        22 => Some(if shift { '^' } else { '6' }),
        23 => Some(if shift { '%' } else { '5' }),
        24 => Some(if shift { '+' } else { '=' }),
        25 => Some(if shift { '(' } else { '9' }),
        26 => Some(if shift { '&' } else { '7' }),
        27 => Some(if shift { '_' } else { '-' }),
        28 => Some(if shift { '*' } else { '8' }),
        29 => Some(if shift { ')' } else { '0' }),
        30 => Some(if shift { '}' } else { ']' }),
        31 => Some(if shift { 'O' } else { 'o' }),
        32 => Some(if shift { 'U' } else { 'u' }),
        33 => Some(if shift { '{' } else { '[' }),
        34 => Some(if shift { 'I' } else { 'i' }),
        35 => Some(if shift { 'P' } else { 'p' }),
        37 => Some(if shift { 'L' } else { 'l' }),
        38 => Some(if shift { 'J' } else { 'j' }),
        39 => Some(if shift { '"' } else { '\'' }),
        40 => Some(if shift { 'K' } else { 'k' }),
        41 => Some(if shift { ':' } else { ';' }),
        42 => Some(if shift { '|' } else { '\\' }),
        43 => Some(if shift { '<' } else { ',' }),
        44 => Some(if shift { '?' } else { '/' }),
        45 => Some(if shift { 'N' } else { 'n' }),
        46 => Some(if shift { 'M' } else { 'm' }),
        47 => Some(if shift { '>' } else { '.' }),
        49 => Some(' '),
        50 => Some(if shift { '~' } else { '`' }),
        _ => None,
    }
}

fn picker_input_from_key(key: &str, shift: bool) -> Option<PickerInput> {
    match key {
        "down" => Some(PickerInput::SelectDown),
        "up" => Some(PickerInput::SelectUp),
        "enter" | "return" => Some(PickerInput::Confirm),
        "escape" | "esc" => Some(PickerInput::Cancel),
        "backspace" | "delete" => Some(PickerInput::SearchBackspace),
        "space" => Some(PickerInput::SearchChar(' ')),
        _ if key.chars().count() == 1 => key.chars().next().map(|ch| {
            let ch = if shift && ch.is_ascii_lowercase() {
                ch.to_ascii_uppercase()
            } else {
                ch
            };
            PickerInput::SearchChar(ch)
        }),
        _ => None,
    }
}

fn picker_key_binding(key: &str, input: PickerInput) -> KeyBinding {
    match input {
        PickerInput::SelectDown => {
            KeyBinding::new(key, SelectDown, Some(WINDOW_PICKER_KEY_CONTEXT))
        }
        PickerInput::SelectUp => KeyBinding::new(key, SelectUp, Some(WINDOW_PICKER_KEY_CONTEXT)),
        PickerInput::ToggleSelect => {
            KeyBinding::new(key, ToggleSelect, Some(WINDOW_PICKER_KEY_CONTEXT))
        }
        PickerInput::Confirm => KeyBinding::new(key, Confirm, Some(WINDOW_PICKER_KEY_CONTEXT)),
        PickerInput::Cancel => KeyBinding::new(key, Cancel, Some(WINDOW_PICKER_KEY_CONTEXT)),
        PickerInput::SearchBackspace | PickerInput::SearchChar(_) => unreachable!(),
    }
}

#[derive(Default)]
pub struct WindowPickerState {
    pub windows: Vec<WindowEntry>,
    pub current_monitor_count: usize,
    pub focused_index: usize,
    pub selected_indices: Vec<usize>,
    pub search_mode: bool,
    pub search_query: String,
    pub search_matches: Vec<usize>,
    pub search_match_index: usize,
    pub previously_focused_window: Option<(i32, u32)>,
    pub window_handle: Option<WindowHandle<PickerContainer>>,
}

impl Global for WindowPickerState {}

type WindowIdentity = (i32, u32);

fn has_secondary_group(state: &WindowPickerState) -> bool {
    state.current_monitor_count > 0 && state.windows.len() > state.current_monitor_count
}

fn visual_row_count(state: &WindowPickerState) -> usize {
    state.windows.len() + usize::from(has_secondary_group(state))
}

fn visual_index_to_window_index(
    visual_index: usize,
    current_monitor_count: usize,
    separator_present: bool,
) -> Option<usize> {
    if !separator_present {
        return Some(visual_index);
    }
    if visual_index == current_monitor_count {
        return None;
    }
    if visual_index > current_monitor_count {
        return Some(visual_index - 1);
    }
    Some(visual_index)
}

fn window_index_to_visual_index(
    window_index: usize,
    current_monitor_count: usize,
    separator_present: bool,
) -> usize {
    if separator_present && window_index >= current_monitor_count {
        window_index + 1
    } else {
        window_index
    }
}

pub struct PickerContainer {
    list: Entity<WindowList>,
    focus_handle: FocusHandle,
}

impl PickerContainer {
    fn new(list: Entity<WindowList>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();
        let blur_handle = focus_handle.clone();
        cx.on_blur(&blur_handle, window, |_this, window, cx| {
            if !is_window_picker_active() {
                return;
            }
            WINDOW_PICKER_ACTIVE.store(false, Ordering::SeqCst);
            cx.update_global::<WindowPickerState, _>(|state, _| {
                state.window_handle = None;
            });
            window.remove_window();
        })
        .detach();
        Self { list, focus_handle }
    }
}

impl Focusable for PickerContainer {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

pub struct WindowList {
    scroll_handle: UniformListScrollHandle,
    last_scrolled_visual_index: Option<usize>,
}

impl WindowList {
    pub fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self {
            scroll_handle: UniformListScrollHandle::new(),
            last_scrolled_visual_index: None,
        }
    }
}

impl Render for WindowList {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = cx.global::<WindowPickerState>();
        let theme = Theme::default();
        let windows = &state.windows;
        let focused_index = state.focused_index;
        let current_monitor_count = state.current_monitor_count;
        let separator_present = has_secondary_group(state);
        let row_count = visual_row_count(state);
        let scroll_handle = self.scroll_handle.clone();

        if windows.is_empty() {
            return div()
                .flex()
                .h(px(100.0))
                .w_full()
                .items_center()
                .justify_center()
                .text_color(theme.muted_foreground)
                .child("No windows on this monitor")
                .into_any();
        }

        let focused_visual_index =
            window_index_to_visual_index(focused_index, current_monitor_count, separator_present);
        if self.last_scrolled_visual_index != Some(focused_visual_index) {
            scroll_handle.scroll_to_item(focused_visual_index, gpui::ScrollStrategy::Top);
            self.last_scrolled_visual_index = Some(focused_visual_index);
        }

        uniform_list(
            cx.entity().clone(),
            "window-list",
            row_count,
            move |_this, range: Range<usize>, _window, cx| {
                let state = cx.global::<WindowPickerState>();
                let theme = Theme::default();
                let windows = &state.windows;
                let focused = state.focused_index;
                let selected = &state.selected_indices;
                let current_monitor_count = state.current_monitor_count;
                let separator_present = has_secondary_group(state);

                range
                    .map(|i| {
                        match visual_index_to_window_index(
                            i,
                            current_monitor_count,
                            separator_present,
                        ) {
                            Some(window_index) => {
                                let win = &windows[window_index];
                                let is_focused = window_index == focused;
                                let is_selected = selected.contains(&window_index);
                                let icon = if let Some(icon_path) = &win.app_icon_path {
                                    img(PathBuf::from(icon_path))
                                        .w(px(16.0))
                                        .h(px(16.0))
                                        .rounded_sm()
                                        .with_fallback(move || {
                                            div()
                                                .w(px(16.0))
                                                .h(px(16.0))
                                                .rounded_sm()
                                                .bg(theme.muted)
                                                .border_1()
                                                .border_color(theme.border)
                                                .into_any_element()
                                        })
                                        .flex_none()
                                        .into_any_element()
                                } else {
                                    div()
                                        .w(px(16.0))
                                        .h(px(16.0))
                                        .rounded_sm()
                                        .bg(theme.muted)
                                        .border_1()
                                        .border_color(theme.border)
                                        .flex_none()
                                        .into_any_element()
                                };

                                div()
                                    .py(px(2.0))
                                    .child(
                                        ListItem::new(window_index)
                                            .selected(is_selected)
                                            .secondary_selected(is_focused)
                                            .on_mouse_enter(move |_ev, _window, cx| {
                                                hover_focus(window_index, cx);
                                            })
                                            .on_click(move |_ev, _window, cx| {
                                                click_select(window_index, cx);
                                            })
                                            .suffix(
                                                div()
                                                    .w(px(140.0))
                                                    .flex_none()
                                                    .overflow_hidden()
                                                    .whitespace_nowrap()
                                                    .text_ellipsis()
                                                    .text_sm()
                                                    .text_color(theme.muted_foreground)
                                                    .child(win.app_name.clone()),
                                            )
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .gap_2()
                                                    .w_full()
                                                    .child(icon)
                                                    .child(
                                                        div()
                                                            .flex_1()
                                                            .min_w(px(0.0))
                                                            .overflow_hidden()
                                                            .whitespace_nowrap()
                                                            .text_ellipsis()
                                                            .text_xs()
                                                            .text_color(theme.foreground)
                                                            .child(win.title.clone()),
                                                    ),
                                            ),
                                    )
                                    .into_any_element()
                            }
                            None => div()
                                .py(px(2.0))
                                .child(
                                    ListItem::new("picker-group-separator").separator().child(
                                        div()
                                            .text_xs()
                                            .text_color(theme.muted_foreground)
                                            .child("Other monitors + minimized"),
                                    ),
                                )
                                .into_any_element(),
                        }
                    })
                    .collect::<Vec<_>>()
            },
        )
        .h_full()
        .track_scroll(scroll_handle)
        .into_any()
    }
}

impl Render for PickerContainer {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::default();
        let state = cx.global::<WindowPickerState>();
        let row_count = visual_row_count(state);

        let height = px((row_count.min(10) as f32 * 40.0 + 60.0).max(160.0));

        let (title, hint) = if state.search_mode {
            let current_hit = if state.search_matches.is_empty() {
                0
            } else {
                state.search_match_index + 1
            };
            (
                format!(
                    "/{} ({}/{})",
                    state.search_query,
                    current_hit,
                    state.search_matches.len()
                ),
                "type to search • enter/esc exit".to_string(),
            )
        } else if state.search_query.is_empty() {
            (
                "Tile windows".to_string(),
                "j/k navigate • space select • / search • n/N next/prev • enter tile • esc cancel"
                    .to_string(),
            )
        } else {
            let current_hit = if state.search_matches.is_empty() {
                0
            } else {
                state.search_match_index + 1
            };
            (
                format!(
                    "Search /{} ({}/{})",
                    state.search_query,
                    current_hit,
                    state.search_matches.len()
                ),
                "".to_string(),
            )
        };

        div()
            .flex()
            .flex_col()
            .h(height)
            .w(px(PICKER_WIDTH))
            .gap_1()
            .rounded_xl()
            .border_1()
            .border_color(theme.border)
            .bg(theme.background)
            .p_2()
            .key_context(WINDOW_PICKER_KEY_CONTEXT)
            .track_focus(&self.focus_handle)
            .on_key_down(
                cx.listener(|_this, event: &gpui::KeyDownEvent, _window, cx| {
                    let key = &event.keystroke.key;
                    let shift = event.keystroke.modifiers.shift;
                    if let Some(input) = picker_input_from_key(key, shift) {
                        handle_picker_input(input, cx);
                        cx.stop_propagation();
                    }
                }),
            )
            .on_action(cx.listener(|_this, _: &SelectDown, _window, cx| {
                handle_picker_input(PickerInput::SelectDown, cx);
            }))
            .on_action(cx.listener(|_this, _: &SelectUp, _window, cx| {
                handle_picker_input(PickerInput::SelectUp, cx);
            }))
            .on_action(cx.listener(|_this, _: &ToggleSelect, _window, cx| {
                handle_picker_input(PickerInput::ToggleSelect, cx);
            }))
            .on_action(cx.listener(|_this, _: &Confirm, _window, cx| {
                handle_picker_input(PickerInput::Confirm, cx);
            }))
            .on_action(cx.listener(|_this, _: &Cancel, _window, cx| {
                handle_picker_input(PickerInput::Cancel, cx);
            }))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .h(px(28.0))
                    .px_2()
                    .text_color(theme.muted_foreground)
                    .child(title)
                    .child(div().text_xs().child(hint)),
            )
            .child(self.list.clone())
            .into_any_element()
    }
}

pub fn handle_picker_input(input: PickerInput, cx: &mut App) {
    if !is_window_picker_active() {
        return;
    }
    let search_mode = cx.global::<WindowPickerState>().search_mode;
    if search_mode {
        match input {
            PickerInput::Confirm | PickerInput::Cancel => exit_search_mode(cx),
            PickerInput::SearchBackspace => pop_search_char(cx),
            PickerInput::SearchChar(ch) => push_search_char(ch, cx),
            _ => {}
        }
        return;
    }
    match input {
        PickerInput::SelectDown => select_down(cx),
        PickerInput::SelectUp => select_up(cx),
        PickerInput::ToggleSelect => toggle_select(cx),
        PickerInput::Confirm => confirm(cx),
        PickerInput::Cancel => cancel(cx),
        PickerInput::SearchBackspace => {}
        PickerInput::SearchChar('/') => enter_search_mode(cx),
        PickerInput::SearchChar('j') => select_down(cx),
        PickerInput::SearchChar('k') => select_up(cx),
        PickerInput::SearchChar(' ') => toggle_select(cx),
        PickerInput::SearchChar('q') => cancel(cx),
        PickerInput::SearchChar('n') => search_next(cx),
        PickerInput::SearchChar('N') => search_previous(cx),
        PickerInput::SearchChar(_) => {}
    }
}

fn matches_query(window: &WindowEntry, query: &str) -> bool {
    if query.is_empty() {
        return false;
    }
    let query = query.to_ascii_lowercase();
    window.app_name.to_ascii_lowercase().contains(&query)
        || window.title.to_ascii_lowercase().contains(&query)
}

fn rebuild_search_matches(state: &mut WindowPickerState) {
    state.search_matches = state
        .windows
        .iter()
        .enumerate()
        .filter_map(|(index, window)| matches_query(window, &state.search_query).then_some(index))
        .collect();

    if state.search_matches.is_empty() {
        state.search_match_index = 0;
        return;
    }

    if let Some(position) = state
        .search_matches
        .iter()
        .position(|index| *index == state.focused_index)
    {
        state.search_match_index = position;
    } else {
        state.search_match_index = 0;
        state.focused_index = state.search_matches[0];
    }
}

fn enter_search_mode(cx: &mut App) {
    cx.update_global::<WindowPickerState, _>(|state, _| {
        state.search_mode = true;
        state.search_query.clear();
        state.search_matches.clear();
        state.search_match_index = 0;
    });
    refresh_window_list(cx);
}

fn exit_search_mode(cx: &mut App) {
    cx.update_global::<WindowPickerState, _>(|state, _| {
        state.search_mode = false;
    });
    refresh_window_list(cx);
}

fn push_search_char(ch: char, cx: &mut App) {
    cx.update_global::<WindowPickerState, _>(|state, _| {
        state.search_query.push(ch);
        rebuild_search_matches(state);
    });
    refresh_window_list(cx);
}

fn pop_search_char(cx: &mut App) {
    cx.update_global::<WindowPickerState, _>(|state, _| {
        state.search_query.pop();
        rebuild_search_matches(state);
    });
    refresh_window_list(cx);
}

fn search_next(cx: &mut App) {
    cx.update_global::<WindowPickerState, _>(|state, _| {
        if state.search_query.is_empty() {
            return;
        }
        if state.search_matches.is_empty() {
            rebuild_search_matches(state);
        }
        if state.search_matches.is_empty() {
            return;
        }
        if let Some(position) = state
            .search_matches
            .iter()
            .position(|index| *index == state.focused_index)
        {
            state.search_match_index = position;
        }
        state.search_match_index = (state.search_match_index + 1) % state.search_matches.len();
        state.focused_index = state.search_matches[state.search_match_index];
    });
    refresh_window_list(cx);
}

fn search_previous(cx: &mut App) {
    cx.update_global::<WindowPickerState, _>(|state, _| {
        if state.search_query.is_empty() {
            return;
        }
        if state.search_matches.is_empty() {
            rebuild_search_matches(state);
        }
        if state.search_matches.is_empty() {
            return;
        }
        if let Some(position) = state
            .search_matches
            .iter()
            .position(|index| *index == state.focused_index)
        {
            state.search_match_index = position;
        }
        if state.search_match_index == 0 {
            state.search_match_index = state.search_matches.len() - 1;
        } else {
            state.search_match_index -= 1;
        }
        state.focused_index = state.search_matches[state.search_match_index];
    });
    refresh_window_list(cx);
}

fn select_down(cx: &mut App) {
    cx.update_global::<WindowPickerState, _>(|state, _| {
        if !state.windows.is_empty() {
            state.focused_index = (state.focused_index + 1) % state.windows.len();
        }
    });
    refresh_window_list(cx);
}

fn select_up(cx: &mut App) {
    cx.update_global::<WindowPickerState, _>(|state, _| {
        if !state.windows.is_empty() {
            if state.focused_index == 0 {
                state.focused_index = state.windows.len() - 1;
            } else {
                state.focused_index -= 1;
            }
        }
    });
    refresh_window_list(cx);
}

fn hover_focus(index: usize, cx: &mut App) {
    let state = cx.global::<WindowPickerState>();
    if index >= state.windows.len() || state.focused_index == index {
        return;
    }
    cx.update_global::<WindowPickerState, _>(|state, _| {
        state.focused_index = index;
    });
    refresh_window_list(cx);
}

fn click_select(index: usize, cx: &mut App) {
    cx.update_global::<WindowPickerState, _>(|state, _| {
        if index >= state.windows.len() {
            return;
        }
        state.focused_index = index;
        if state.selected_indices.contains(&index) {
            state.selected_indices.retain(|i| *i != index);
        } else {
            state.selected_indices.push(index);
        }
    });
    refresh_window_list(cx);
}

fn toggle_select(cx: &mut App) {
    cx.update_global::<WindowPickerState, _>(|state, _| {
        if state.selected_indices.contains(&state.focused_index) {
            state.selected_indices.retain(|i| *i != state.focused_index);
        } else {
            state.selected_indices.push(state.focused_index);
        }
    });
    refresh_window_list(cx);
}

fn confirm(cx: &mut App) {
    let (windows_to_tile, previously_focused_window): (
        Vec<WindowIdentity>,
        Option<WindowIdentity>,
    ) = {
        let state = cx.global::<WindowPickerState>();
        let indices = if state.selected_indices.is_empty() {
            vec![state.focused_index]
        } else {
            state.selected_indices.clone()
        };
        let windows = indices
            .into_iter()
            .filter_map(|i| state.windows.get(i))
            .map(|w| (w.pid, w.window_id))
            .collect();
        (windows, state.previously_focused_window)
    };

    close_picker(cx);

    if windows_to_tile.is_empty() {
        if let Some((pid, window_id)) = previously_focused_window {
            let _ = focus_saved_window(pid, window_id);
        }
        return;
    }

    if !windows_to_tile.is_empty()
        && let Ok(screens) = get_screens()
        && let Some(main_screen) = screens.iter().find(|s| s.is_main)
    {
        let _ = tile_windows_in_columns(&windows_to_tile, main_screen);
    }

    for (pid, window_id) in &windows_to_tile {
        let _ = focus_saved_window(*pid, *window_id);
    }

    let target = previously_focused_window
        .filter(|focused| windows_to_tile.contains(focused))
        .or_else(|| windows_to_tile.first().copied());

    if let Some((pid, window_id)) = target {
        let _ = focus_saved_window(pid, window_id);
    }
}

fn cancel(cx: &mut App) {
    let previously_focused_window = cx.global::<WindowPickerState>().previously_focused_window;
    close_picker(cx);
    if let Some((pid, window_id)) = previously_focused_window {
        let _ = focus_saved_window(pid, window_id);
    }
}

pub fn show_window_picker(cx: &mut App) {
    WINDOW_PICKER_ACTIVE.store(false, Ordering::SeqCst);
    let screens = match get_screens() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to get screens: {}", e);
            return;
        }
    };

    let all_windows = match get_all_windows() {
        Ok(w) => w,
        Err(e) => {
            eprintln!("Failed to get all windows: {}", e);
            return;
        }
    };

    let focused_window_rect = get_focused_window()
        .ok()
        .and_then(|window| get_window_rect(&window).ok());

    let previously_focused_window = focused_window_rect.as_ref().and_then(|window_rect| {
        window_rect
            .window_id
            .map(|window_id| (window_rect.pid, window_id))
    });

    let current_screen = focused_window_rect
        .as_ref()
        .and_then(|window_rect| get_screen_for_window(window_rect).ok())
        .or_else(|| screens.iter().find(|s| s.is_main).cloned())
        .or_else(|| screens.first().cloned());
    let current_screen = match current_screen {
        Some(screen) => screen,
        None => {
            eprintln!("No screens found");
            return;
        }
    };

    let mut current_monitor_windows = Vec::new();
    let mut secondary_windows = Vec::new();
    for window in all_windows {
        let (x, y, width, height) = window.bounds;
        let center_x = x + width / 2.0;
        let center_y = y + height / 2.0;
        if center_x >= current_screen.x
            && center_x < current_screen.x + current_screen.width
            && center_y >= current_screen.y
            && center_y < current_screen.y + current_screen.height
        {
            current_monitor_windows.push(window);
        } else {
            secondary_windows.push(window);
        }
    }

    let current_monitor_count = current_monitor_windows.len();
    let mut windows = current_monitor_windows;
    windows.extend(secondary_windows);

    let row_count = windows.len()
        + usize::from(current_monitor_count > 0 && windows.len() > current_monitor_count);
    let selected_indices = if let Some((_, id)) = previously_focused_window
        && let Some(index) = windows.iter().position(|w| w.window_id == id)
    {
        vec![index]
    } else {
        vec![]
    };

    cx.set_global(WindowPickerState {
        windows,
        current_monitor_count,
        focused_index: selected_indices.first().cloned().unwrap_or_default(),
        selected_indices,
        search_mode: false,
        search_query: String::new(),
        search_matches: Vec::new(),
        search_match_index: 0,
        previously_focused_window,
        window_handle: None,
    });

    let height = (row_count.min(10) as f32 * 40.0 + 60.0).max(160.0);
    let y_offset = ((current_screen.height - height as f64) / 2.0) as f32;
    let x_center = (current_screen.x + (current_screen.width - PICKER_WIDTH as f64) / 2.0) as f32;
    let y_center = (current_screen.y + y_offset as f64) as f32;

    let window_handle = cx.open_window(
        WindowOptions {
            titlebar: None,
            window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                gpui::Point::new(px(x_center), px(y_center)),
                Size {
                    width: px(PICKER_WIDTH),
                    height: px(height),
                },
            ))),
            window_background: WindowBackgroundAppearance::Blurred,
            kind: WindowKind::PopUp,
            ..Default::default()
        },
        |window, cx| {
            let list = cx.new(|cx| WindowList::new(window, cx));
            cx.new(|cx| PickerContainer::new(list, window, cx))
        },
    );

    match window_handle {
        Ok(handle) => {
            WINDOW_PICKER_ACTIVE.store(true, Ordering::SeqCst);
            cx.update_global::<WindowPickerState, _>(|state, _| {
                state.window_handle = Some(handle);
            });
            let _ = handle.update(cx, |container, window, _cx| {
                window.activate_window();
                window.focus(&container.focus_handle);
            });
        }
        Err(e) => {
            WINDOW_PICKER_ACTIVE.store(false, Ordering::SeqCst);
            eprintln!("Failed to open picker window: {}", e);
        }
    }
}

fn close_picker(cx: &mut App) {
    WINDOW_PICKER_ACTIVE.store(false, Ordering::SeqCst);
    let window = cx.global::<WindowPickerState>().window_handle;

    if let Some(window) = window {
        let _ = window.update(cx, |_, window, _cx| {
            window.remove_window();
        });
    }
}

fn refresh_window_list(cx: &mut App) {
    let handle = cx.global::<WindowPickerState>().window_handle;
    if let Some(handle) = handle {
        let _ = handle.update(cx, |container, _window, cx| {
            container.list.update(cx, |_, cx| cx.notify());
        });
    }
}

fn focus_saved_window(pid: i32, window_id: u32) -> bool {
    let window = match find_window_by_id(pid, window_id) {
        Ok(window) => window,
        Err(e) => {
            eprintln!(
                "Failed to find window for focus restoration (pid={}, id={}): {}",
                pid, window_id, e
            );
            return false;
        }
    };

    if let Err(e) = focus_window(&window) {
        eprintln!(
            "Failed to focus window during picker restore (pid={}, id={}): {}",
            pid, window_id, e
        );
        return false;
    }

    true
}

//! macOS Accessibility API wrappers for window management

use accessibility::{AXAttribute, AXUIElement, AXUIElementAttributes};
use accessibility_sys::{
    AXIsProcessTrusted, AXIsProcessTrustedWithOptions, AXUIElementGetPid, AXUIElementRef,
    AXUIElementSetAttributeValue,
};
use core_foundation::base::{CFType, TCFType};

use core_foundation::boolean::CFBoolean;
use core_foundation::dictionary::CFDictionary;

use core_foundation::number::CFNumber;
use core_foundation::string::CFString;
use core_graphics::window::CGWindowID;
use std::time::Duration;

use crate::error::PixieError;

/// Test if the Accessibility API actually works (not just permissions check)
pub fn test_api_access() -> Result<(), PixieError> {
    if !has_accessibility_permissions() {
        return Err(PixieError::Accessibility(
            "Process is not trusted for Accessibility (AXIsProcessTrusted=false)".to_string(),
        ));
    }
    Ok(())
}

/// Check if the app has Accessibility permissions
pub fn has_accessibility_permissions() -> bool {
    unsafe { AXIsProcessTrusted() }
}

/// Request Accessibility permissions from the user
pub fn request_accessibility_permissions() -> bool {
    unsafe {
        let key = CFString::new("AXTrustedCheckOptionPrompt");
        let value = CFBoolean::true_value();
        let dict = CFDictionary::from_CFType_pairs(&[(key.as_CFType(), value.as_CFType())]);
        AXIsProcessTrustedWithOptions(dict.as_CFTypeRef() as *const _)
    }
}

#[cfg(target_os = "macos")]
fn frontmost_application_pid() -> Option<i32> {
    use core_graphics::window::{
        create_description_from_array, create_window_list, kCGNullWindowID, kCGWindowLayer,
        kCGWindowListExcludeDesktopElements, kCGWindowListOptionOnScreenOnly, kCGWindowOwnerPID,
    };

    let options = kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements;
    let window_ids = create_window_list(options, kCGNullWindowID)?;
    let descriptions = create_description_from_array(window_ids)?;
    let layer_key = unsafe { CFString::wrap_under_get_rule(kCGWindowLayer) };
    let owner_pid_key = unsafe { CFString::wrap_under_get_rule(kCGWindowOwnerPID) };

    for i in 0..descriptions.len() {
        let Some(window) = descriptions.get(i) else {
            continue;
        };

        let layer = window
            .find(&layer_key)
            .and_then(|v| v.downcast::<CFNumber>())
            .and_then(|n| n.to_i64())
            .unwrap_or(-1);
        if layer != 0 {
            continue;
        }

        if let Some(pid) = window
            .find(&owner_pid_key)
            .and_then(|v| v.downcast::<CFNumber>())
            .and_then(|n| n.to_i32())
        {
            return Some(pid);
        }
    }

    None
}

#[cfg(not(target_os = "macos"))]
fn frontmost_application_pid() -> Option<i32> {
    None
}

/// Get the currently focused window as an AXUIElement
pub fn get_focused_window() -> Result<AXUIElement, PixieError> {
    if !has_accessibility_permissions() {
        return Err(PixieError::Accessibility(
            "Accessibility permission missing for current process".to_string(),
        ));
    }

    let system = AXUIElement::system_wide();
    let focused_window_attr: AXAttribute<CFType> =
        AXAttribute::new(&CFString::new("AXFocusedWindow"));
    let focused_app_attr: AXAttribute<CFType> =
        AXAttribute::new(&CFString::new("AXFocusedApplication"));

    let window_server_err = if let Some(pid) = frontmost_application_pid() {
        let app_element = AXUIElement::application(pid);
        match app_element.attribute(&focused_window_attr) {
            Ok(window_value) => {
                if let Some(window) = window_value.downcast_into::<AXUIElement>() {
                    return Ok(window);
                }
                "Window-server PID path returned non-window value".to_string()
            }
            Err(e) => format!("Window-server PID {} AXFocusedWindow error: {:?}", pid, e),
        }
    } else {
        "Window-server PID unavailable".to_string()
    };

    // Prefer focused application -> focused window to avoid stale window reads.
    let focused_app_err = match system.attribute(&focused_app_attr) {
        Ok(app_value) => {
            if let Some(app_element) = app_value.downcast_into::<AXUIElement>() {
                match app_element.attribute(&focused_window_attr) {
                    Ok(window_value) => {
                        if let Some(window) = window_value.downcast_into::<AXUIElement>() {
                            return Ok(window);
                        }
                        "Focused app window value is not an AXUIElement".to_string()
                    }
                    Err(e) => format!("Focused app AXFocusedWindow error: {:?}", e),
                }
            } else {
                "AXFocusedApplication is not an AXUIElement".to_string()
            }
        }
        Err(e) => format!("AXFocusedApplication error: {:?}", e),
    };

    // Some contexts return AXError::AttributeUnsupported for AXFocusedWindow on system-wide element.
    // Fallback to AXFocusedUIElement and resolve its AXWindow / AXParent chain.
    let system_focused_window_err = match system.attribute(&focused_window_attr) {
        Ok(value) => {
            return value.downcast_into::<AXUIElement>().ok_or_else(|| {
                PixieError::Accessibility(
                    "System AXFocusedWindow is not an AXUIElement".to_string(),
                )
            });
        }
        Err(e) => format!("{:?}", e),
    };

    let focused_ui_attr: AXAttribute<CFType> =
        AXAttribute::new(&CFString::new("AXFocusedUIElement"));
    let focused_ui_value = system.attribute(&focused_ui_attr).map_err(|e| {
        PixieError::Accessibility(format!(
            "Failed to resolve focused window (window-server path: {}; focused-app path: {}; AXFocusedWindow error: {}; AXFocusedUIElement error: {:?})",
            window_server_err, focused_app_err, system_focused_window_err, e
        ))
    })?;

    let focused_ui = focused_ui_value
        .downcast_into::<AXUIElement>()
        .ok_or_else(|| {
            PixieError::Accessibility("AXFocusedUIElement is not an AXUIElement".to_string())
        })?;

    let window_attr: AXAttribute<CFType> = AXAttribute::new(&CFString::new("AXWindow"));
    if let Ok(value) = focused_ui.attribute(&window_attr) {
        if let Some(window) = value.downcast_into::<AXUIElement>() {
            return Ok(window);
        }
    }

    if focused_ui
        .role()
        .map(|role| role == "AXWindow")
        .unwrap_or(false)
    {
        return Ok(focused_ui);
    }

    let parent_attr: AXAttribute<CFType> = AXAttribute::new(&CFString::new("AXParent"));
    let mut current = focused_ui;
    for _ in 0..8 {
        let Ok(parent_value) = current.attribute(&parent_attr) else {
            break;
        };

        let Some(parent) = parent_value.downcast_into::<AXUIElement>() else {
            break;
        };

        if parent
            .role()
            .map(|role| role == "AXWindow")
            .unwrap_or(false)
        {
            return Ok(parent);
        }

        current = parent;
    }

    Err(PixieError::Accessibility(format!(
        "Failed to resolve focused window (window-server path: {}; focused-app path: {}; AXFocusedWindow error: {})",
        window_server_err, focused_app_err, system_focused_window_err
    )))
}

/// Get the currently focused window, retrying for transient failures.
pub fn get_focused_window_with_retry(
    max_attempts: u32,
    retry_delay: Duration,
) -> Result<AXUIElement, PixieError> {
    let attempts = max_attempts.max(1);
    let mut last_error = "unknown error".to_string();

    for attempt in 1..=attempts {
        match get_focused_window() {
            Ok(window) => return Ok(window),
            Err(err) => {
                last_error = err.to_string();
                if attempt < attempts {
                    std::thread::sleep(retry_delay);
                }
            }
        }
    }

    Err(PixieError::Accessibility(format!(
        "Failed to get focused window after {} attempts: {}",
        attempts, last_error
    )))
}

/// Information about a window
#[derive(Debug, Clone)]
pub struct WindowInfo {
    pub pid: i32,
    pub title: String,
    pub role: String,
}

impl WindowInfo {
    pub fn display_string(&self) -> String {
        let app_name = get_app_name(self.pid).unwrap_or_else(|_| "Unknown".to_string());
        if self.title.is_empty() {
            format!("{} (PID: {})", app_name, self.pid)
        } else {
            format!("{} - \"{}\" (PID: {})", app_name, self.title, self.pid)
        }
    }
}

/// Get information about a window element
pub fn get_window_info(element: &AXUIElement) -> Result<WindowInfo, PixieError> {
    let title = element.title().map(|s| s.to_string()).unwrap_or_default();

    let role = element.role().map(|s| s.to_string()).unwrap_or_default();

    let pid = get_pid(element)?;

    Ok(WindowInfo { pid, title, role })
}

/// Get the PID from an AXUIElement
fn get_pid(element: &AXUIElement) -> Result<i32, PixieError> {
    unsafe {
        let mut pid: i32 = 0;
        let element_ref = element.as_concrete_TypeRef();
        let result = AXUIElementGetPid(element_ref, &mut pid);

        if result != 0 {
            Err(PixieError::Accessibility("Failed to get PID".to_string()))
        } else {
            Ok(pid)
        }
    }
}

/// Focus a window by bringing its application to front and making the window main
pub fn focus_window(element: &AXUIElement) -> Result<(), PixieError> {
    let pid = get_pid(element)?;
    let app_element = AXUIElement::application(pid);

    unsafe {
        let attr = CFString::new("AXFrontmost");
        let value = CFBoolean::true_value();

        let result = AXUIElementSetAttributeValue(
            app_element.as_concrete_TypeRef(),
            attr.as_concrete_TypeRef(),
            value.as_CFTypeRef(),
        );

        if result != 0 {
            return Err(PixieError::Accessibility(format!(
                "Failed to bring app to front: {}",
                result
            )));
        }
    }

    element
        .set_main(CFBoolean::true_value())
        .map_err(|e| PixieError::Accessibility(format!("Failed to set window as main: {:?}", e)))?;

    let _ = element.perform_action(&CFString::new("AXRaise"));

    Ok(())
}

/// Get the application name from a PID
pub fn get_app_name(pid: i32) -> Result<String, PixieError> {
    use std::process::Command;

    let output = Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "comm="])
        .output();

    match output {
        Ok(output) => Ok(String::from_utf8_lossy(&output.stdout).trim().to_string()),
        Err(_) => Ok("Unknown".to_string()),
    }
}

/// Get the CGWindowID from an AXUIElement
pub fn get_window_id(element: &AXUIElement) -> Result<CGWindowID, PixieError> {
    extern "C" {
        fn _AXUIElementGetWindow(element: AXUIElementRef, window_id: *mut u32) -> i32;
    }

    unsafe {
        let mut window_id: u32 = 0;
        let result = _AXUIElementGetWindow(element.as_concrete_TypeRef(), &mut window_id);

        if result == 0 && window_id != 0 {
            Ok(window_id)
        } else {
            Err(PixieError::Accessibility(
                "Failed to get window ID".to_string(),
            ))
        }
    }
}

/// Find a window by PID and window ID
pub fn find_window_by_id(pid: i32, window_id: CGWindowID) -> Result<AXUIElement, PixieError> {
    let app_element = AXUIElement::application(pid);

    let windows = app_element
        .windows()
        .map_err(|e| PixieError::Accessibility(format!("Failed to get windows: {:?}", e)))?;

    for i in 0..windows.len() {
        if let Some(window) = windows.get(i) {
            let window = window.clone();
            if let Ok(id) = get_window_id(&window) {
                if id == window_id {
                    return Ok(window);
                }
            }
        }
    }

    Err(PixieError::WindowNotFound)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Left,
    Right,
    Up,
    Down,
}

#[derive(Debug, Clone)]
pub struct WindowRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub element: AXUIElement,
    pub pid: i32,
    pub window_id: Option<u32>,
}

pub fn get_window_rect(element: &AXUIElement) -> Result<WindowRect, PixieError> {
    use accessibility_sys::AXValueGetValue;
    use core_graphics::geometry::{CGPoint, CGRect, CGSize};

    let frame_attr: AXAttribute<CFType> = AXAttribute::new(&CFString::new("AXFrame"));

    let frame_value = element
        .attribute(&frame_attr)
        .map_err(|e| PixieError::Accessibility(format!("Failed to get AXFrame: {:?}", e)))?;

    let mut rect = CGRect::new(&CGPoint::new(0.0, 0.0), &CGSize::new(0.0, 0.0));
    let success = unsafe {
        AXValueGetValue(
            frame_value.as_CFTypeRef() as *mut _,
            accessibility_sys::kAXValueTypeCGRect,
            &mut rect as *mut _ as *mut _,
        )
    };

    if !success {
        return Err(PixieError::Accessibility(
            "Failed to extract CGRect from AXValue".to_string(),
        ));
    }

    let pid = get_pid(element)?;
    let window_id = get_window_id(element).ok();

    Ok(WindowRect {
        x: rect.origin.x,
        y: rect.origin.y,
        width: rect.size.width,
        height: rect.size.height,
        element: element.clone(),
        pid,
        window_id,
    })
}

pub fn find_window_in_direction(
    from: &WindowRect,
    direction: Direction,
) -> Result<AXUIElement, PixieError> {
    use core_foundation::number::CFNumber;
    use core_graphics::window::{
        create_description_from_array, create_window_list, kCGNullWindowID, kCGWindowBounds,
        kCGWindowLayer, kCGWindowListExcludeDesktopElements, kCGWindowListOptionOnScreenOnly,
        kCGWindowOwnerPID,
    };

    let options = kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements;
    let window_ids = create_window_list(options, kCGNullWindowID)
        .ok_or_else(|| PixieError::Accessibility("Failed to get window list".to_string()))?;
    let descriptions = create_description_from_array(window_ids).ok_or_else(|| {
        PixieError::Accessibility("Failed to get window descriptions".to_string())
    })?;

    let layer_key = unsafe { CFString::wrap_under_get_rule(kCGWindowLayer) };
    let owner_pid_key = unsafe { CFString::wrap_under_get_rule(kCGWindowOwnerPID) };
    let bounds_key = unsafe { CFString::wrap_under_get_rule(kCGWindowBounds) };
    let window_number_key = CFString::new("kCGWindowNumber");

    let mut scored_candidates: Vec<(f64, usize, i32, u32)> = Vec::new();

    for i in 0..descriptions.len() {
        let Some(window_desc) = descriptions.get(i) else {
            continue;
        };

        let layer = window_desc
            .find(&layer_key)
            .and_then(|v| v.downcast::<CFNumber>())
            .and_then(|n| n.to_i64())
            .unwrap_or(-1);
        if layer != 0 {
            continue;
        }

        let Some(pid) = window_desc
            .find(&owner_pid_key)
            .and_then(|v| v.downcast::<CFNumber>())
            .and_then(|n| n.to_i32())
        else {
            continue;
        };

        let window_id: Option<u32> = window_desc
            .find(&window_number_key)
            .and_then(|v| v.downcast::<CFNumber>())
            .and_then(|n| n.to_i64())
            .map(|n| n as u32);

        if pid == from.pid {
            if let (Some(wid), Some(from_wid)) = (window_id, from.window_id) {
                if wid == from_wid {
                    continue;
                }
            }
        }

        // Get bounds from the window description
        let bounds_value = window_desc.find(&bounds_key);
        let bounds_dict = match bounds_value {
            Some(v) => match v.downcast::<CFDictionary>() {
                Some(d) => d,
                None => continue,
            },
            None => continue,
        };

        // Extract bounds values
        let x = get_dict_f64(&bounds_dict, "X");
        let y = get_dict_f64(&bounds_dict, "Y");
        let width = get_dict_f64(&bounds_dict, "Width");
        let height = get_dict_f64(&bounds_dict, "Height");

        if let Some(score) =
            calculate_direction_score_simple(from, (x, y, width, height), direction)
        {
            if let Some(wid) = window_id {
                scored_candidates.push((score, i as usize, pid, wid));
            }
        }
    }

    scored_candidates.sort_by(|a, b| match a.0.partial_cmp(&b.0) {
        Some(std::cmp::Ordering::Equal) | None => a.1.cmp(&b.1),
        Some(ord) => ord,
    });

    if let Some((_, _, pid, window_id)) = scored_candidates.into_iter().next() {
        return find_window_element_by_id(pid, window_id);
    }

    Err(PixieError::WindowNotFound)
}

fn find_window_element_by_id(pid: i32, window_id: u32) -> Result<AXUIElement, PixieError> {
    let app_element = AXUIElement::application(pid);

    let windows = app_element
        .windows()
        .map_err(|e| PixieError::Accessibility(format!("Failed to get windows: {:?}", e)))?;

    for i in 0..windows.len() {
        if let Some(win) = windows.get(i) {
            let win = win.clone();
            if let Ok(win_id) = get_window_id(&win) {
                if win_id == window_id {
                    return Ok(win);
                }
            }
        }
    }

    // Fallback: return first window of the app
    if let Some(win) = windows.get(0) {
        return Ok(win.clone());
    }

    Err(PixieError::Accessibility(format!(
        "Could not find window element for pid={}, window_id={}",
        pid, window_id
    )))
}

fn calculate_direction_score_simple(
    from: &WindowRect,
    other_bounds: (f64, f64, f64, f64),
    direction: Direction,
) -> Option<f64> {
    let (other_x, other_y, other_width, other_height) = other_bounds;

    // Check if candidate window extends past the current window in the target direction
    // This allows partially overlapping windows as long as they extend further in that direction
    match direction {
        Direction::Left => {
            // Candidate's left edge must be left of current's left edge
            if other_x >= from.x {
                return None;
            }
        }
        Direction::Right => {
            // Candidate's right edge must be right of current's right edge
            if other_x + other_width <= from.x + from.width {
                return None;
            }
        }
        Direction::Up => {
            // Candidate's top edge must be above current's top edge
            if other_y >= from.y {
                return None;
            }
        }
        Direction::Down => {
            // Candidate's bottom edge must be below current's bottom edge
            if other_y + other_height <= from.y + from.height {
                return None;
            }
        }
    }

    // Calculate the distance from current window's edge to the candidate's edge
    // For overlapping windows, distance can be negative, which we treat as 0
    // (they're visually adjacent, just overlapping)
    let (primary_distance, overlap) = match direction {
        Direction::Left => {
            // Distance from current left edge to candidate's right edge
            let dist = from.x - (other_x + other_width);
            let ov = overlap_amount_1d(
                from.y,
                from.y + from.height,
                other_y,
                other_y + other_height,
            );
            (dist, ov)
        }
        Direction::Right => {
            // Distance from current right edge to candidate's left edge
            let dist = other_x - (from.x + from.width);
            let ov = overlap_amount_1d(
                from.y,
                from.y + from.height,
                other_y,
                other_y + other_height,
            );
            (dist, ov)
        }
        Direction::Up => {
            // Distance from current top edge to candidate's bottom edge
            let dist = from.y - (other_y + other_height);
            let ov = overlap_amount_1d(from.x, from.x + from.width, other_x, other_x + other_width);
            (dist, ov)
        }
        Direction::Down => {
            // Distance from current bottom edge to candidate's top edge
            let dist = other_y - (from.y + from.height);
            let ov = overlap_amount_1d(from.x, from.x + from.width, other_x, other_x + other_width);
            (dist, ov)
        }
    };

    // For overlapping windows (negative distance), treat as distance 0
    // They're still valid candidates since they extend in the target direction
    let primary_distance = primary_distance.max(0.0);

    let overlap_bonus = overlap * 100.0;
    Some(primary_distance - overlap_bonus)
}

fn overlap_amount_1d(a1: f64, a2: f64, b1: f64, b2: f64) -> f64 {
    (a2.min(b2) - a1.max(b1)).max(0.0)
}

fn get_dict_f64(dict: &CFDictionary, key: &str) -> f64 {
    let key = CFString::new(key);
    unsafe {
        let mut value: *const std::ffi::c_void = std::ptr::null();
        let key_ptr = key.as_CFTypeRef() as *const std::ffi::c_void;
        if core_foundation::dictionary::CFDictionaryGetValueIfPresent(
            dict.as_concrete_TypeRef(),
            key_ptr,
            &mut value,
        ) != 0
            && !value.is_null()
        {
            let cf_type = CFType::wrap_under_get_rule(value);
            cf_type
                .downcast::<CFNumber>()
                .map(|n| n.to_f64().unwrap_or(0.0))
                .unwrap_or(0.0)
        } else {
            0.0
        }
    }
}

#[derive(Debug, Clone)]
pub struct Screen {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub is_main: bool,
}

pub fn get_screens() -> Result<Vec<Screen>, PixieError> {
    use core_graphics::display::CGDisplay;

    let display_ids = CGDisplay::active_displays().map_err(|e| {
        PixieError::Accessibility(format!("Failed to get active displays: {:?}", e))
    })?;

    let screens: Vec<Screen> = display_ids
        .into_iter()
        .map(|id| {
            let display = CGDisplay::new(id);
            let bounds = display.bounds();
            Screen {
                x: bounds.origin.x,
                y: bounds.origin.y,
                width: bounds.size.width,
                height: bounds.size.height,
                is_main: display.is_main(),
            }
        })
        .collect();

    if screens.is_empty() {
        return Err(PixieError::Accessibility(
            "No active displays found".to_string(),
        ));
    }

    Ok(screens)
}

pub fn get_screen_for_window(window_rect: &WindowRect) -> Result<Screen, PixieError> {
    let screens = get_screens()?;

    let window_center_x = window_rect.x + window_rect.width / 2.0;
    let window_center_y = window_rect.y + window_rect.height / 2.0;

    for screen in &screens {
        if window_center_x >= screen.x
            && window_center_x < screen.x + screen.width
            && window_center_y >= screen.y
            && window_center_y < screen.y + screen.height
        {
            return Ok(screen.clone());
        }
    }

    let mut closest_screen = screens.first().cloned().unwrap();
    let mut min_distance = f64::MAX;

    for screen in &screens {
        let screen_center_x = screen.x + screen.width / 2.0;
        let screen_center_y = screen.y + screen.height / 2.0;
        let distance = (window_center_x - screen_center_x).powi(2)
            + (window_center_y - screen_center_y).powi(2);
        if distance < min_distance {
            min_distance = distance;
            closest_screen = screen.clone();
        }
    }

    Ok(closest_screen)
}

pub fn set_window_rect(
    element: &AXUIElement,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> Result<(), PixieError> {
    use accessibility_sys::AXValueCreate;
    use core_graphics::geometry::{CGPoint, CGSize};

    unsafe {
        let position = CGPoint::new(x, y);
        let position_value = AXValueCreate(
            accessibility_sys::kAXValueTypeCGPoint,
            &position as *const _ as *const _,
        );
        if position_value.is_null() {
            return Err(PixieError::Accessibility(
                "Failed to create AXValue for position".to_string(),
            ));
        }

        let attr = CFString::new("AXPosition");
        let result = AXUIElementSetAttributeValue(
            element.as_concrete_TypeRef(),
            attr.as_concrete_TypeRef(),
            position_value as *const _,
        );

        core_foundation::base::CFRelease(position_value as *const _);

        if result != 0 {
            return Err(PixieError::Accessibility(format!(
                "Failed to set window position: {}",
                result
            )));
        }

        let size = CGSize::new(width, height);
        let size_value = AXValueCreate(
            accessibility_sys::kAXValueTypeCGSize,
            &size as *const _ as *const _,
        );
        if size_value.is_null() {
            return Err(PixieError::Accessibility(
                "Failed to create AXValue for size".to_string(),
            ));
        }

        let attr = CFString::new("AXSize");
        let result = AXUIElementSetAttributeValue(
            element.as_concrete_TypeRef(),
            attr.as_concrete_TypeRef(),
            size_value as *const _,
        );

        core_foundation::base::CFRelease(size_value as *const _);

        if result != 0 {
            return Err(PixieError::Accessibility(format!(
                "Failed to set window size: {}",
                result
            )));
        }
    }

    Ok(())
}

pub fn minimize_window(element: &AXUIElement) -> Result<(), PixieError> {
    unsafe {
        let attr = CFString::new("AXMinimized");
        let value = CFBoolean::true_value();

        let result = AXUIElementSetAttributeValue(
            element.as_concrete_TypeRef(),
            attr.as_concrete_TypeRef(),
            value.as_CFTypeRef(),
        );

        if result != 0 {
            return Err(PixieError::Accessibility(format!(
                "Failed to minimize window: {}",
                result
            )));
        }
    }

    Ok(())
}

pub fn maximize_window(element: &AXUIElement) -> Result<(), PixieError> {
    let window_rect = get_window_rect(element)?;
    let screen = get_screen_for_window(&window_rect)?;

    let menu_bar_height = if screen.is_main { 25.0 } else { 0.0 };

    let dock_height = get_dock_height()?;

    let available_x = screen.x;
    let available_y = screen.y + menu_bar_height;
    let available_width = screen.width;
    let available_height = screen.height - menu_bar_height - dock_height;

    set_window_rect(
        element,
        available_x,
        available_y,
        available_width,
        available_height,
    )
}

fn get_dock_height() -> Result<f64, PixieError> {
    use std::process::Command;

    let output = Command::new("defaults")
        .args(["read", "com.apple.dock", "orientation"])
        .output();

    let orientation = match output {
        Ok(o) => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        Err(_) => "bottom".to_string(),
    };

    let autohide_output = Command::new("defaults")
        .args(["read", "com.apple.dock", "autohide"])
        .output();

    let autohide = match autohide_output {
        Ok(o) => String::from_utf8_lossy(&o.stdout).trim() == "1",
        Err(_) => false,
    };

    if autohide {
        return Ok(0.0);
    }

    match orientation.as_str() {
        "bottom" => Ok(80.0),
        "left" | "right" => Ok(0.0),
        _ => Ok(80.0),
    }
}

pub fn toggle_fullscreen(element: &AXUIElement) -> Result<(), PixieError> {
    let fullscreen_attr: AXAttribute<CFType> = AXAttribute::new(&CFString::new("AXFullScreen"));

    let current_value = element
        .attribute(&fullscreen_attr)
        .map_err(|e| PixieError::Accessibility(format!("Failed to get AXFullScreen: {:?}", e)))?;

    let is_fullscreen = current_value
        .downcast::<CFBoolean>()
        .map(|b| b == CFBoolean::true_value())
        .unwrap_or(false);

    let new_value = if is_fullscreen {
        CFBoolean::false_value()
    } else {
        CFBoolean::true_value()
    };

    unsafe {
        let attr = CFString::new("AXFullScreen");
        let result = AXUIElementSetAttributeValue(
            element.as_concrete_TypeRef(),
            attr.as_concrete_TypeRef(),
            new_value.as_CFTypeRef(),
        );

        if result != 0 {
            return Err(PixieError::Accessibility(format!(
                "Failed to toggle fullscreen: {}",
                result
            )));
        }
    }

    Ok(())
}

pub fn center_window(element: &AXUIElement) -> Result<(), PixieError> {
    let window_rect = get_window_rect(element)?;
    let screen = get_screen_for_window(&window_rect)?;

    let menu_bar_height = if screen.is_main { 25.0 } else { 0.0 };

    let available_x = screen.x;
    let available_y = screen.y + menu_bar_height;
    let available_width = screen.width;
    let available_height = screen.height - menu_bar_height;

    let new_x = available_x + (available_width - window_rect.width) / 2.0;
    let new_y = available_y + (available_height - window_rect.height) / 2.0;

    set_window_rect(element, new_x, new_y, window_rect.width, window_rect.height)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MonitorDirection {
    Left,
    Right,
    Up,
    Down,
}

pub fn move_window_to_monitor(
    element: &AXUIElement,
    direction: MonitorDirection,
) -> Result<(), PixieError> {
    let window_rect = get_window_rect(element)?;
    let current_screen = get_screen_for_window(&window_rect)?;
    let screens = get_screens()?;

    let target_screen = find_adjacent_screen(&current_screen, &screens, direction)?;

    let rel_left = (window_rect.x - current_screen.x) / current_screen.width;
    let rel_top = (window_rect.y - current_screen.y) / current_screen.height;
    let rel_width = window_rect.width / current_screen.width;
    let rel_height = window_rect.height / current_screen.height;

    let new_x = target_screen.x + rel_left * target_screen.width;
    let new_y = target_screen.y + rel_top * target_screen.height;
    let new_width = rel_width * target_screen.width;
    let new_height = rel_height * target_screen.height;

    set_window_rect(element, new_x, new_y, new_width, new_height)
}

fn find_adjacent_screen(
    current: &Screen,
    screens: &[Screen],
    direction: MonitorDirection,
) -> Result<Screen, PixieError> {
    let current_center_x = current.x + current.width / 2.0;
    let current_center_y = current.y + current.height / 2.0;

    let candidates: Vec<(f64, &Screen)> = screens
        .iter()
        .filter(|s| {
            let is_different = (s.x - current.x).abs() > 1.0 || (s.y - current.y).abs() > 1.0;
            is_different
        })
        .filter_map(|s| {
            let screen_center_x = s.x + s.width / 2.0;
            let screen_center_y = s.y + s.height / 2.0;

            let dx = screen_center_x - current_center_x;
            let dy = screen_center_y - current_center_y;

            let is_in_direction = match direction {
                MonitorDirection::Left => dx < 0.0 && dx.abs() > dy.abs(),
                MonitorDirection::Right => dx > 0.0 && dx.abs() > dy.abs(),
                MonitorDirection::Up => dy < 0.0 && dy.abs() > dx.abs(),
                MonitorDirection::Down => dy > 0.0 && dy.abs() > dx.abs(),
            };

            if is_in_direction {
                let distance = (dx * dx + dy * dy).sqrt();
                Some((distance, s))
            } else {
                None
            }
        })
        .collect();

    if let Some((_, screen)) = candidates
        .iter()
        .min_by(|a, b| a.0.partial_cmp(&b.0).unwrap())
    {
        return Ok((*screen).clone());
    }

    let fallback = screens
        .iter()
        .find(|s| (s.x - current.x).abs() > 1.0 || (s.y - current.y).abs() > 1.0)
        .cloned();

    fallback.ok_or_else(|| PixieError::Accessibility("No adjacent monitor found".to_string()))
}

pub fn apply_placement(
    element: &AXUIElement,
    placement: &crate::config::Placement,
) -> Result<(), PixieError> {
    let window_rect = get_window_rect(element)?;
    let screen = get_screen_for_window(&window_rect)?;

    let menu_bar_height = if screen.is_main { 25.0 } else { 0.0 };

    let available_x = screen.x;
    let available_y = screen.y + menu_bar_height;
    let available_width = screen.width;
    let available_height = screen.height - menu_bar_height;

    let new_width = match &placement.width {
        Some(w) => crate::config::parse_size_value(w, available_width)?,
        None => window_rect.width,
    };

    let new_height = match &placement.height {
        Some(h) => crate::config::parse_size_value(h, available_height)?,
        None => window_rect.height,
    };

    let new_x = match &placement.left {
        Some(l) => {
            available_x + crate::config::parse_position_value(l, available_width, new_width)?
        }
        None => window_rect.x,
    };

    let new_y = match &placement.top {
        Some(t) => {
            available_y + crate::config::parse_position_value(t, available_height, new_height)?
        }
        None => window_rect.y,
    };

    set_window_rect(element, new_x, new_y, new_width, new_height)
}

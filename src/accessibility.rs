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
        create_description_from_array, create_window_list, kCGNullWindowID, kCGWindowLayer,
        kCGWindowListExcludeDesktopElements, kCGWindowListOptionOnScreenOnly, kCGWindowOwnerPID,
        kCGWindowBounds,
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

    let windows = app_element.windows()
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
        ) != 0 && !value.is_null()
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

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

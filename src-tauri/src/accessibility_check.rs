#[cfg(target_os = "macos")]
pub mod macos_focus {
    use objc2::msg_send;
    use objc2::rc::autoreleasepool;
    use objc2::runtime::{AnyClass, AnyObject};
    use objc2_foundation::NSString;
    use std::ffi::c_void;

    #[repr(C)]
    struct __AXUIElement([u8; 0]);
    type AXUIElementRef = *const __AXUIElement;
    type AXError = i32;
    const K_AX_ERROR_SUCCESS: AXError = 0;

    #[link(name = "ApplicationServices", kind = "framework")]
    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn AXUIElementCreateSystemWide() -> AXUIElementRef;
        fn AXUIElementCreateApplication(pid: i32) -> AXUIElementRef;
        fn AXUIElementCopyAttributeValue(
            element: AXUIElementRef,
            attribute: *const c_void,
            value: *mut *const c_void,
        ) -> AXError;
        fn CFRelease(cf: *const c_void);
        fn CFGetTypeID(cf: *const c_void) -> usize;
        fn CFStringGetTypeID() -> usize;
    }

    pub fn has_focused_text_element() -> bool {
        autoreleasepool(|_| unsafe {
            let ws_class = match AnyClass::get(c"NSWorkspace") {
                Some(cls) => cls,
                None => return true,
            };

            let ws: *mut AnyObject = msg_send![ws_class, sharedWorkspace];
            if ws.is_null() {
                return true;
            }

            let front_app: *mut AnyObject = msg_send![ws, frontmostApplication];
            if front_app.is_null() {
                // No frontmost app -> desktop/empty -> copy to clipboard
                return false;
            }

            let bundle_id_ns: *mut NSString = msg_send![front_app, bundleIdentifier];
            let pid: i32 = msg_send![front_app, processIdentifier];

            let bundle_id = if !bundle_id_ns.is_null() {
                (&*bundle_id_ns).to_string().to_lowercase()
            } else {
                String::new()
            };

            // 1. Text-centric apps, AI assistants, IDEs, and Terminals:
            // These apps are explicitly designed for text input and editing.
            // Accessibility APIs often return disabled/internal states in Electron/Chromium,
            // but keystroke injection (Cmd+V) always works.
            if bundle_id.contains("antigravity")
                || bundle_id.contains("claude")
                || bundle_id.contains("vscode")
                || bundle_id.contains("cursor")
                || bundle_id.contains("terminal")
                || bundle_id.contains("iterm")
                || bundle_id.contains("warp")
                || bundle_id.contains("ghostty")
                || bundle_id.contains("kitty")
                || bundle_id.contains("alacritty")
                || bundle_id.contains("sublime")
                || bundle_id.contains("jetbrains")
                || bundle_id.contains("zed")
            {
                return true;
            }

            // 2. System Shell & Desktop:
            // When user clicks the desktop wallpaper or dock, Finder / Dock is frontmost.
            let is_system_shell = bundle_id == "com.apple.finder"
                || bundle_id == "com.apple.dock"
                || bundle_id == "com.apple.windowmanager"
                || bundle_id == "com.apple.systemuiserver"
                || bundle_id == "com.apple.notificationcenterui";

            // Helper to inspect focused element attributes
            let inspect_element = |elem: AXUIElementRef| -> Option<bool> {
                // Check AXSelectedTextRange: universal indicator of text cursor / selection
                let attr_range = NSString::from_str("AXSelectedTextRange");
                let mut range_val: *const c_void = std::ptr::null();
                let range_err = AXUIElementCopyAttributeValue(
                    elem,
                    &*attr_range as *const NSString as *const c_void,
                    &mut range_val,
                );
                if !range_val.is_null() {
                    CFRelease(range_val);
                }
                if range_err == K_AX_ERROR_SUCCESS {
                    return Some(true);
                }

                // Check AXRole
                let attr_role = NSString::from_str("AXRole");
                let mut role_val: *const c_void = std::ptr::null();
                let role_err = AXUIElementCopyAttributeValue(
                    elem,
                    &*attr_role as *const NSString as *const c_void,
                    &mut role_val,
                );
                if role_err == K_AX_ERROR_SUCCESS && !role_val.is_null() {
                    let role_str_opt = if CFGetTypeID(role_val) == CFStringGetTypeID() {
                        Some((&*(role_val as *const NSString)).to_string())
                    } else {
                        None
                    };
                    CFRelease(role_val);

                    if let Some(role_str) = role_str_opt {
                        if role_str == "AXTextField"
                            || role_str == "AXTextArea"
                            || role_str == "AXComboBox"
                            || role_str == "AXSearchField"
                        {
                            return Some(true);
                        }

                        // Explicit non-text UI controls
                        if role_str == "AXButton"
                            || role_str == "AXCheckBox"
                            || role_str == "AXRadioButton"
                            || role_str == "AXSlider"
                            || role_str == "AXScrollBar"
                            || role_str == "AXTabGroup"
                            || role_str == "AXMenuBar"
                            || role_str == "AXMenu"
                            || role_str == "AXMenuItem"
                            || role_str == "AXToolbar"
                        {
                            return Some(false);
                        }

                        // For system shell (Desktop/Finder), scroll/web areas have no typing target
                        if is_system_shell && (role_str == "AXScrollArea" || role_str == "AXWebArea") {
                            return Some(false);
                        }
                    }
                }

                None
            };

            // Query application-specific AXUIElement
            let app_elem = if pid > 0 {
                AXUIElementCreateApplication(pid)
            } else {
                std::ptr::null()
            };

            let mut focused_val: *const c_void = std::ptr::null();
            let attr_focused = NSString::from_str("AXFocusedUIElement");

            let mut query_success = false;
            if !app_elem.is_null() {
                let err = AXUIElementCopyAttributeValue(
                    app_elem,
                    &*attr_focused as *const NSString as *const c_void,
                    &mut focused_val,
                );
                CFRelease(app_elem as *const c_void);
                if err == K_AX_ERROR_SUCCESS && !focused_val.is_null() {
                    query_success = true;
                }
            }

            if !query_success {
                let system_wide = AXUIElementCreateSystemWide();
                if !system_wide.is_null() {
                    let err = AXUIElementCopyAttributeValue(
                        system_wide,
                        &*attr_focused as *const NSString as *const c_void,
                        &mut focused_val,
                    );
                    CFRelease(system_wide as *const c_void);
                    if err == K_AX_ERROR_SUCCESS && !focused_val.is_null() {
                        query_success = true;
                    }
                }
            }

            let elem_result = if query_success && !focused_val.is_null() {
                let result = inspect_element(focused_val as AXUIElementRef);
                CFRelease(focused_val);
                result
            } else {
                None
            };

            if let Some(is_text) = elem_result {
                return is_text;
            }

            // If Handy itself is frontmost and no focused text element was found,
            // do not inject keystrokes into the app UI; copy to clipboard instead.
            if bundle_id == "com.pais.handy" {
                return false;
            }

            // If we are in the system shell (Desktop, Dock) with no text field -> clipboard
            if is_system_shell {
                return false;
            }

            // Fail-open for general apps so typing is never broken
            true
        })
    }
}

#[cfg(target_os = "macos")]
pub use macos_focus::has_focused_text_element;

#[cfg(not(target_os = "macos"))]
pub fn has_focused_text_element() -> bool {
    true
}

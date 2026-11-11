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

            let is_system_shell = bundle_id == "com.apple.finder"
                || bundle_id == "com.apple.dock"
                || bundle_id == "com.apple.windowmanager"
                || bundle_id == "com.apple.systemuiserver"
                || bundle_id == "com.apple.notificationcenterui";

            let is_handy = bundle_id == "com.pais.handy" || bundle_id == "com.pais.handy.dev";

            // For all third-party apps (browsers, editors, canvas apps like Muse, terminals, etc.),
            // return true immediately so Cmd+V keystroke injection is always attempted just like upstream.
            if !is_system_shell && !is_handy {
                return true;
            }

            // For macOS system shell (Desktop, Dock) or Handy itself, verify if an editable text element
            // (e.g. renaming a file in Finder, Finder search bar, Handy settings text input) is focused.
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

            if query_success && !focused_val.is_null() {
                let attr_role = NSString::from_str("AXRole");
                let mut role_val: *const c_void = std::ptr::null();
                let role_err = AXUIElementCopyAttributeValue(
                    focused_val as AXUIElementRef,
                    &*attr_role as *const NSString as *const c_void,
                    &mut role_val,
                );
                CFRelease(focused_val);

                if role_err == K_AX_ERROR_SUCCESS && !role_val.is_null() {
                    let is_text_role = if CFGetTypeID(role_val) == CFStringGetTypeID() {
                        let role_str = (&*(role_val as *const NSString)).to_string();
                        role_str == "AXTextField"
                            || role_str == "AXTextArea"
                            || role_str == "AXComboBox"
                            || role_str == "AXSearchField"
                    } else {
                        false
                    };
                    CFRelease(role_val);
                    return is_text_role;
                }
            }

            // System shell or Handy with no focused text input -> copy to clipboard
            false
        })
    }
}

#[cfg(target_os = "macos")]
pub use macos_focus::has_focused_text_element;

#[cfg(not(target_os = "macos"))]
pub fn has_focused_text_element() -> bool {
    true
}

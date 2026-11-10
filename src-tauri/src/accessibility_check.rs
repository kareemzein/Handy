#[cfg(target_os = "macos")]
pub mod macos_focus {
    use objc2::msg_send;
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
        fn AXUIElementCopyAttributeValue(
            element: AXUIElementRef,
            attribute: *const c_void,
            value: *mut *const c_void,
        ) -> AXError;
        fn CFRelease(cf: *const c_void);
    }

    pub fn has_focused_text_element() -> bool {
        unsafe {
            let system_wide = AXUIElementCreateSystemWide();
            if system_wide.is_null() {
                return false;
            }

            let attr_focused = NSString::from_str("AXFocusedUIElement");
            let mut focused_val: *const c_void = std::ptr::null();
            let err = AXUIElementCopyAttributeValue(
                system_wide,
                &*attr_focused as *const NSString as *const c_void,
                &mut focused_val,
            );
            CFRelease(system_wide as *const c_void);

            if err != K_AX_ERROR_SUCCESS || focused_val.is_null() {
                return false;
            }

            let focused_elem = focused_val as AXUIElementRef;

            // 1. Check if the element has an active text selection / insertion point range
            // AXSelectedTextRange is the universal attribute supported by all editable text fields
            let attr_range = NSString::from_str("AXSelectedTextRange");
            let mut range_val: *const c_void = std::ptr::null();
            let range_err = AXUIElementCopyAttributeValue(
                focused_elem,
                &*attr_range as *const NSString as *const c_void,
                &mut range_val,
            );
            if !range_val.is_null() {
                CFRelease(range_val);
            }
            if range_err == K_AX_ERROR_SUCCESS {
                CFRelease(focused_val);
                return true;
            }

            // 2. Check role (AXTextField, AXTextArea, AXComboBox, AXSearchField)
            let attr_role = NSString::from_str("AXRole");
            let mut role_val: *const c_void = std::ptr::null();
            let role_err = AXUIElementCopyAttributeValue(
                focused_elem,
                &*attr_role as *const NSString as *const c_void,
                &mut role_val,
            );

            let mut is_text_role = false;
            if role_err == K_AX_ERROR_SUCCESS && !role_val.is_null() {
                let role_ns = &*(role_val as *const NSString);
                let role_str = role_ns.to_string();
                if role_str == "AXTextField"
                    || role_str == "AXTextArea"
                    || role_str == "AXComboBox"
                    || role_str == "AXSearchField"
                {
                    is_text_role = true;
                }
                CFRelease(role_val);
            }

            CFRelease(focused_val);

            if is_text_role {
                return true;
            }

            // 3. Terminal apps (which accept direct text input even without AXSelectedTextRange)
            if let Some(ws_class) = AnyClass::get(c"NSWorkspace") {
                let ws: *mut AnyObject = msg_send![ws_class, sharedWorkspace];
                if !ws.is_null() {
                    let front_app: *mut AnyObject = msg_send![ws, frontmostApplication];
                    if !front_app.is_null() {
                        let bundle_id: *mut NSString = msg_send![front_app, bundleIdentifier];
                        if !bundle_id.is_null() {
                            let b = (&*bundle_id).to_string();
                            let terminals = [
                                "com.apple.Terminal",
                                "com.googlecode.iterm2",
                                "dev.warp.Warp-Stable",
                                "com.mitchellh.ghostty",
                                "net.kovidgoyal.kitty",
                                "com.alacritty",
                            ];
                            for t in &terminals {
                                if b.eq_ignore_ascii_case(t) {
                                    return true;
                                }
                            }
                        }
                    }
                }
            }

            false
        }
    }
}

#[cfg(target_os = "macos")]
pub use macos_focus::has_focused_text_element;

#[cfg(not(target_os = "macos"))]
pub fn has_focused_text_element() -> bool {
    true
}

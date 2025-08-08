use std::ffi::{c_int, c_uint, c_void};

use crate::utils::*;

static mut EVENT_TAP: *mut c_void = std::ptr::null_mut();

// --- CoreGraphics FFI for global hotkey registration ---
#[link(name = "CoreGraphics", kind = "framework")]
#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CGEventTapCreate(
        tap: c_uint,
        place: c_uint,
        options: c_uint,
        events_of_interest: u64,
        callback: CGEventTapCallBack,
        refcon: *mut c_void,
    ) -> *mut c_void;

    fn CGEventTapEnable(tap: *mut c_void, enable: bool);

    fn CFRunLoopGetCurrent() -> *mut c_void;
    fn CFRunLoopAddSource(rl: *mut c_void, source: *mut c_void, mode: *mut c_void);
    fn CFMachPortCreateRunLoopSource(
        allocator: *mut c_void,
        port: *mut c_void,
        order: c_int,
    ) -> *mut c_void;

    fn CGEventGetFlags(event: *mut c_void) -> u64;
    fn CGEventGetIntegerValueField(event: *mut c_void, field: c_uint) -> i64;

    static kCFRunLoopCommonModes: *mut c_void;
}

type CGEventTapCallBack = extern "C" fn(
    proxy: *mut c_void,
    event_type: c_uint,
    event: *mut c_void,
    refcon: *mut c_void,
) -> *mut c_void;

// Constants for CGEventTap
const K_CG_SESSION_EVENT_TAP: c_uint = 0;
const K_CG_HEAD_INSERT_EVENT_TAP: c_uint = 0;
const K_CG_EVENT_TAP_OPTION_DEFAULT: c_uint = 0;
const K_CG_EVENT_KEY_DOWN: c_uint = 10;
const K_CG_KEYCODE_FIELD: c_uint = 9;

// Modifier flags
const K_CG_EVENT_FLAG_MASK_COMMAND: u64 = 0x100000;
const K_CG_EVENT_FLAG_MASK_SHIFT: u64 = 0x20000;
// This is the callback for the hotkey event
extern "C" fn event_tap_callback(
    _proxy: *mut c_void,
    event_type: c_uint,
    event: *mut c_void,
    _refcon: *mut c_void,
) -> *mut c_void {
    unsafe {
        if event_type == K_CG_EVENT_KEY_DOWN {
            let keycode = CGEventGetIntegerValueField(event, K_CG_KEYCODE_FIELD);
            let flags = CGEventGetFlags(event);

            // Check for Cmd+Shift+K (keycode 40)
            if keycode == 40
                && (flags & K_CG_EVENT_FLAG_MASK_COMMAND) != 0
                && (flags & K_CG_EVENT_FLAG_MASK_SHIFT) != 0
            {
                ll("🎯 HOTKEY PRESSED! Toggling window visibility...");

                // Dispatch to main thread
                let delegate_ptr = crate::APP_DELEGATE.load(std::sync::atomic::Ordering::SeqCst);
                if !delegate_ptr.is_null() {
                    let delegate = &*delegate_ptr;
                    let selector = objc2::sel!(showEguiWindow);
                    let _: () = objc2::msg_send![delegate, performSelectorOnMainThread: selector, withObject: std::ptr::null::<objc2_foundation::NSObject>(), waitUntilDone:false];
                } else {
                    ll("❌ No delegate available for dispatch");
                }

                // Return null to consume the event (prevent it from propagating)
                return std::ptr::null_mut();
            }
        }
    }

    // Return the event unchanged for other keys
    event
}

pub(crate) unsafe fn register_hotkey() {
    ll("🪧 Setting up CGEventTap for global hotkey...");

    // Create event mask for key down events
    let event_mask = 1u64 << K_CG_EVENT_KEY_DOWN;

    let event_tap = unsafe {
        CGEventTapCreate(
            K_CG_SESSION_EVENT_TAP,
            K_CG_HEAD_INSERT_EVENT_TAP,
            K_CG_EVENT_TAP_OPTION_DEFAULT,
            event_mask,
            event_tap_callback,
            std::ptr::null_mut(),
        )
    };

    if event_tap.is_null() {
        ll("🪧 Failed to create event tap! You may need to grant Accessibility permissions:");
        ll("\tSystem Settings > Privacy & Security > Accessibility");
        ll("\tAdd your terminal app or the popup binary to the list.");
        return;
    }

    EVENT_TAP = event_tap;
    ll("✅ Event tap created successfully!");

    // Create a run loop source for the event tap
    let run_loop_source =
        unsafe { CFMachPortCreateRunLoopSource(std::ptr::null_mut(), event_tap, 0) };

    if run_loop_source.is_null() {
        ll("❌ Failed to create run loop source!");
        return;
    }

    // Add the source to the current run loop
    let current_run_loop = unsafe { CFRunLoopGetCurrent() };
    unsafe {
        CFRunLoopAddSource(current_run_loop, run_loop_source, kCFRunLoopCommonModes);
    }

    // Enable the event tap
    unsafe {
        CGEventTapEnable(event_tap, true);
    }

    ll("🎯 Global hotkey registered successfully!");
    ll("🪧 Press Cmd+Shift+K to toggle the popup window");
}

use objc2::{msg_send, sel};
use objc2::{rc::Retained, runtime::AnyObject};
use objc2_foundation::{NSObject, NSString};
use std::ffi::c_int;
use std::sync::atomic::Ordering;

#[link(name = "Foundation", kind = "framework")]
unsafe extern "C" {
    unsafe fn NSLog(format: *const objc2::runtime::AnyObject, ...);
}

pub(crate) fn ll(msg: &str) {
    let ns_msg: Retained<AnyObject> = NSString::from_str(msg).into();
    unsafe {
        NSLog(Retained::as_ptr(&ns_msg));
    }
}

// Signal handler for SIGINT (Ctrl+C)
pub(crate) extern "C" fn sigint_handler(_signal: c_int) {
    ll("🪧 Received SIGINT (Ctrl+C) - initiating proper app termination...");

    let app_ptr = super::APP_INSTANCE.load(Ordering::SeqCst);
    if !app_ptr.is_null() {
        unsafe {
            let app = &*app_ptr;
            // Use performSelectorOnMainThread to ensure thread safety
            let selector = sel!(terminate:);
            let _: () = msg_send![app, performSelectorOnMainThread: selector, withObject:std::ptr::null::<NSObject>(), waitUntilDone:false];
            ll("✅ Cocoa exit.");
        }
    } else {
        ll("☑ No app instance available, exiting directly");
        std::process::exit(0);
    }
}

pub(crate) unsafe fn setup_signal_handler() {
    unsafe extern "C" {
        fn signal(sig: c_int, handler: extern "C" fn(c_int)) -> extern "C" fn(c_int);
    }

    const SIGINT: c_int = 2;
    unsafe {
        signal(SIGINT, sigint_handler);
    }
    ll("✅ Set up SIGINT handler for graceful Ctrl+C handling");
}

#![allow(unsafe_op_in_unsafe_fn)]

use crate::hotkey::{APP_DELEGATE, AppDelegate};
use objc2::runtime::ProtocolObject;
use objc2::{MainThreadMarker, rc::Retained};
use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};

use std::sync::atomic::{AtomicPtr, Ordering};

mod blitzortung;
mod hotkey;
mod trrpy;
mod ui;
mod utils;
mod websocket;

// Global reference to NSApplication for signal handler
static APP_INSTANCE: AtomicPtr<NSApplication> = AtomicPtr::new(std::ptr::null_mut());

fn main() {
    let mtm: MainThreadMarker = MainThreadMarker::new().unwrap();

    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Regular);

    // Store app reference for signal handler
    APP_INSTANCE.store(
        Retained::as_ptr(&app) as *mut NSApplication,
        Ordering::SeqCst,
    );

    // Set up signal handler for graceful Ctrl+C handling
    unsafe {
        utils::setup_signal_handler();
    }

    // Configure the application delegate
    let delegate = AppDelegate::new(mtm);
    let object = ProtocolObject::from_ref(&*delegate);
    app.setDelegate(Some(object));

    // Store delegate reference for hotkey dispatching
    APP_DELEGATE.store(
        Retained::as_ptr(&delegate) as *mut AppDelegate,
        Ordering::SeqCst,
    );

    // Run the app
    app.run();
}

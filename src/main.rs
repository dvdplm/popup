#![allow(unsafe_op_in_unsafe_fn)]
mod hotkey;
mod utils;

use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::{MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy, NSApplicationDelegate};
use objc2_foundation::{NSNotification, NSObject, NSObjectProtocol};
use std::sync::atomic::{AtomicPtr, Ordering};

use eframe::egui;

use utils::*;

// Global reference to NSApplication for signal handler
static APP_INSTANCE: AtomicPtr<NSApplication> = AtomicPtr::new(std::ptr::null_mut());

// Global reference to AppDelegate for hotkey dispatching
pub(crate) static APP_DELEGATE: AtomicPtr<AppDelegate> = AtomicPtr::new(std::ptr::null_mut());

define_class!(
    // SAFETY:
    // - The superclass NSObject does not have any subclassing requirements.
    // - `AppDelegate` does not implement `Drop`.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    struct AppDelegate;

    unsafe impl NSObjectProtocol for AppDelegate {}

    unsafe impl NSApplicationDelegate for AppDelegate {
        #[unsafe(method(applicationDidFinishLaunching:))]
        fn did_finish_launching(&self, notification: &NSNotification) {
            ll("🪧 Did finish launching!");
            ll(&format!("🪧 Process ID: {}", std::process::id()));
            // Do something with the notification
            dbg!(notification);

            // Register the global hotkey (Cmd+Shift+M)
            unsafe {
                hotkey::register_hotkey();
            }
        }

        #[unsafe(method(applicationWillTerminate:))]
        fn will_terminate(&self, _notification: &NSNotification) {
            ll("🪧 Will terminate!");
        }

        #[unsafe(method(showEguiWindow))]
        fn show_egui_window(&self) {
            ll("🎯 Main thread here! Ready to show egui window");
            // TODO: Initialize and show eframe/egui window here
        }
    }
);

impl AppDelegate {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm);
        let this = this.set_ivars(());
        unsafe { msg_send![super(this), init] }
    }
}

fn main() -> eframe::Result {
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

    // configure the application delegate
    let delegate = AppDelegate::new(mtm);
    let object = ProtocolObject::from_ref(&*delegate);
    app.setDelegate(Some(object));

    // Store delegate reference for hotkey dispatching
    APP_DELEGATE.store(
        Retained::as_ptr(&delegate) as *mut AppDelegate,
        Ordering::SeqCst,
    );

    // run the app
    app.run();
    Ok(())
}

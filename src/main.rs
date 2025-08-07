#![allow(unsafe_op_in_unsafe_fn)]
mod egui_view;
mod hotkey;
mod trrpy;
mod utils;

use crate::egui_view::EguiView;
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::{MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSApplicationDelegate, NSBackingStoreType,
    NSWindow, NSWindowDelegate, NSWindowStyleMask,
};
use objc2_foundation::{
    NSNotification, NSObject, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString,
};
use std::sync::atomic::{AtomicPtr, Ordering};

use utils::*;

// Global reference to NSApplication for signal handler
static APP_INSTANCE: AtomicPtr<NSApplication> = AtomicPtr::new(std::ptr::null_mut());

// Global reference to AppDelegate for hotkey dispatching
pub(crate) static APP_DELEGATE: AtomicPtr<AppDelegate> = AtomicPtr::new(std::ptr::null_mut());

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    struct WindowDelegate;

    unsafe impl NSObjectProtocol for WindowDelegate {}

    unsafe impl NSWindowDelegate for WindowDelegate {
        #[unsafe(method(windowShouldClose:))]
        fn window_should_close(&self, _sender: &NSWindow) -> bool {
            ll("🚪 Window close button clicked - terminating app...");
            let mtm = MainThreadMarker::new().unwrap();
            let app = NSApplication::sharedApplication(mtm);
            unsafe {
                app.terminate(None);
            }
            false // We terminate the app, so don't close the window normally
        }

        #[unsafe(method(windowWillClose:))]
        fn window_will_close(&self, _notification: &NSNotification) {
            ll("🪟 Window will close notification");
        }
    }
);

impl WindowDelegate {
    fn new(mtm: MainThreadMarker) -> objc2::rc::Retained<Self> {
        let this = Self::alloc(mtm);
        let this = this.set_ivars(());
        unsafe { msg_send![super(this), init] }
    }
}

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

        #[unsafe(method(applicationShouldTerminate:))]
        fn should_terminate(
            &self,
            _sender: &NSApplication,
        ) -> objc2_app_kit::NSApplicationTerminateReply {
            ll("🪧 Application should terminate - cleaning up resources...");

            // Give wgpu time to clean up by letting the current frame finish
            std::thread::sleep(std::time::Duration::from_millis(16));

            objc2_app_kit::NSApplicationTerminateReply::TerminateNow
        }

        #[unsafe(method(applicationWillTerminate:))]
        fn will_terminate(&self, _notification: &NSNotification) {
            ll("🪧 Application will terminate - final cleanup...");

            // Allow any pending operations to complete
            std::thread::sleep(std::time::Duration::from_millis(50));

            ll("✅ Final cleanup complete - goodbye!");
        }

        #[unsafe(method(showEguiWindow))]
        fn show_egui_window(&self) {
            ll("🎯 Main thread here! Creating egui window...");

            // Get the MainThreadMarker since we are on the main thread.
            let mtm = MainThreadMarker::from(self);

            // First, activate the application to bring it to focus - use aggressive activation
            ll("🔍 Activating application with force...");
            let app = NSApplication::sharedApplication(mtm);
            // Use the old method that forces activation even when another app is active
            app.activateIgnoringOtherApps(true);

            // For now, we create a new window every time. A real app would likely
            // want to cache and reuse the window.
            let frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(400.0, 300.0));
            let style_mask = NSWindowStyleMask::Titled
                | NSWindowStyleMask::Closable
                | NSWindowStyleMask::Resizable;
            let backing_store_type = NSBackingStoreType::Buffered;

            // Use the alloc/init pattern for creating a window with parameters.
            let window = unsafe {
                let w = NSWindow::alloc(mtm);
                NSWindow::initWithContentRect_styleMask_backing_defer(
                    w,
                    frame,
                    style_mask,
                    backing_store_type,
                    false, // defer
                )
            };
            let title = NSString::from_str("Trrpy");
            window.setTitle(&title);
            window.center();

            // Set window level to floating to ensure it appears above other apps
            ll("🔝 Setting window level to floating...");
            window.setLevel(3); // NSFloatingWindowLevel = 3

            // Create and set window delegate to handle close events
            let window_delegate = WindowDelegate::new(mtm);
            window.setDelegate(Some(objc2::runtime::ProtocolObject::from_ref(
                &*window_delegate,
            )));

            // Store the delegate to prevent deallocation
            std::mem::forget(window_delegate);

            // Create our custom egui view
            let view = EguiView::new(mtm);

            // Set the view as the window's content view
            window.setContentView(Some(&view));

            // IMPORTANT: Initialize the egui/wgpu state *after* the view is in the window.
            view.init_state();

            // Show and focus the window
            ll("🪟 Making window key and ordering front...");
            window.makeKeyAndOrderFront(None);

            // Ensure the window is at the front and focused
            ll("🔝 Bringing window to front regardless...");
            unsafe {
                window.orderFrontRegardless();
            }

            // Make the view the first responder so it can receive keyboard events immediately
            ll("⌨️ Setting first responder...");
            window.makeFirstResponder(Some(&view));

            // Additional focus methods to ensure we get focus from other apps
            ll("🔄 Performing additional focus operations...");

            // Bring all app windows to front
            unsafe {
                let _: () = objc2::msg_send![&app, arrangeInFront: std::ptr::null::<objc2_foundation::NSObject>()];
            }

            // Force focus on our specific window
            unsafe {
                window.orderWindow_relativeTo(objc2_app_kit::NSWindowOrderingMode::Above, 0);
            }

            // Request attention to make the app icon bounce in the dock
            ll("🔔 Requesting user attention...");
            app.requestUserAttention(objc2_app_kit::NSRequestUserAttentionType::CriticalRequest);

            // Center the window on screen for better visibility
            ll("🎯 Centering window...");
            window.center();

            // Final activation to ensure focus
            app.activateIgnoringOtherApps(true);

            // Add small delay to allow focus changes to take effect
            ll("⏱️ Allowing focus changes to process...");
            std::thread::sleep(std::time::Duration::from_millis(50));

            // Final key window operation to ensure focus
            ll("🔑 Final key window operation...");
            window.makeKeyAndOrderFront(None);

            ll("✅ Window setup and aggressive focusing complete!");
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
}

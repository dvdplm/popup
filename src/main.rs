#![allow(unsafe_op_in_unsafe_fn)]
mod egui_view;
mod hotkey;
mod trrpy;
mod utils;

use crate::egui_view::EguiView;
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSApplicationDelegate, NSBackingStoreType,
    NSWindow, NSWindowDelegate, NSWindowStyleMask,
};
use objc2_foundation::{NSNotification, NSObject, NSObjectProtocol, NSPoint, NSRect, NSSize};
use std::sync::atomic::{AtomicPtr, Ordering};

use utils::*;

// Global reference to NSApplication for signal handler
static APP_INSTANCE: AtomicPtr<NSApplication> = AtomicPtr::new(std::ptr::null_mut());

// Global reference to AppDelegate for hotkey dispatching
pub(crate) static APP_DELEGATE: AtomicPtr<AppDelegate> = AtomicPtr::new(std::ptr::null_mut());

#[derive(Debug)]
pub(crate) struct Ivars {
    window: Option<objc2::rc::Retained<NSWindow>>,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    struct WindowDelegate;

    unsafe impl NSObjectProtocol for WindowDelegate {}

    unsafe impl NSWindowDelegate for WindowDelegate {
        #[unsafe(method(windowShouldClose:))]
        fn window_should_close(&self, sender: &NSWindow) -> bool {
            ll("🚪 Window close requested - hiding window...");
            sender.orderOut(None);
            false // Don't actually close the window, just hide it
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
    #[ivars = Ivars]
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
            ll("🪧 Application should terminate - hiding window and allowing exit...");

            // Hide the window if it exists
            if let Some(ref window) = self.ivars().window {
                window.orderOut(None);
            }

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
            // Get the MainThreadMarker since we are on the main thread.
            let mtm = MainThreadMarker::from(self);

            // Check if we already have a window
            if let Some(ref window) = self.ivars().window {
                if window.isVisible() {
                    ll("🙈 Window is visible, hiding it...");
                    window.orderOut(None);
                    return;
                } else {
                    ll("👁️ Window exists but hidden, showing it...");
                    unsafe {
                        let _: () = objc2::msg_send![self, showExistingWindow: &**window];
                    }
                    return;
                }
            }

            ll("🎯 Creating new egui window...");

            // First, activate the application to bring it to focus - use aggressive activation
            ll("🔍 Activating application with force...");
            let app = NSApplication::sharedApplication(mtm);
            // Use the old method that forces activation even when another app is active
            app.activateIgnoringOtherApps(true);

            // Create a borderless window for popup-style UI
            let frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(400.0, 300.0));
            let style_mask = NSWindowStyleMask::Borderless;
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
            // No title for borderless window
            window.center();

            // Set window level to floating to ensure it appears above other apps
            ll("🔝 Setting window level to floating...");
            window.setLevel(3); // NSFloatingWindowLevel = 3

            // Enable mouse moved events for borderless window
            window.setAcceptsMouseMovedEvents(true);

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

            // Store the window reference for future show/hide operations
            // Safety: We need to get a mutable reference to the ivars
            let ivars_ptr = self.ivars() as *const Ivars as *mut Ivars;
            unsafe {
                (*ivars_ptr).window = Some(window.clone());
            }

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
            #[allow(deprecated)]
            app.activateIgnoringOtherApps(true);

            // Add small delay to allow focus changes to take effect
            ll("⏱️ Allowing focus changes to process...");
            std::thread::sleep(std::time::Duration::from_millis(50));

            // Final key window operation to ensure focus
            ll("🔑 Final key window operation...");
            window.makeKeyAndOrderFront(None);

            ll("✅ Window setup and aggressive focusing complete!");
        }

        #[unsafe(method(showExistingWindow:))]
        fn show_existing_window(&self, window: &NSWindow) {
            ll("🔍 Showing existing window with aggressive focus...");

            // Get the MainThreadMarker since we are on the main thread.
            let mtm = MainThreadMarker::from(self);
            let app = NSApplication::sharedApplication(mtm);

            // Aggressive activation
            #[allow(deprecated)]
            app.activateIgnoringOtherApps(true);

            // Show and focus the window
            window.makeKeyAndOrderFront(None);

            unsafe {
                window.orderFrontRegardless();
            }

            // Additional focus operations
            unsafe {
                let _: () = objc2::msg_send![&app, arrangeInFront: std::ptr::null::<objc2_foundation::NSObject>()];
                window.orderWindow_relativeTo(objc2_app_kit::NSWindowOrderingMode::Above, 0);
            }

            // Make first responder
            if let Some(content_view) = window.contentView() {
                window.makeFirstResponder(Some(&content_view));
            }

            // // Request attention
            // app.requestUserAttention(objc2_app_kit::NSRequestUserAttentionType::CriticalRequest);

            // Allow focus changes to process
            std::thread::sleep(std::time::Duration::from_millis(50));

            // Final activation
            #[allow(deprecated)]
            app.activateIgnoringOtherApps(true);
            window.makeKeyAndOrderFront(None);

            ll("✅ Existing window focused!");
        }
    }
);

impl AppDelegate {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm);
        let this = this.set_ivars(Ivars { window: None });
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

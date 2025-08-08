use crate::trrpy::TrrpyApp;
use crate::ui::EguiView;
use crate::utils::*;
use objc2::rc::Retained;
use objc2::runtime::{AnyClass, AnyObject};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSApplication, NSApplicationDelegate, NSBackingStoreType, NSView, NSWindow, NSWindowDelegate,
    NSWindowStyleMask, NSWorkspace,
};
use objc2_foundation::{NSNotification, NSObject, NSObjectProtocol, NSPoint, NSRect, NSSize};
use std::cell::RefMut;
use std::ffi::{CStr, c_int, c_uint, c_void};
use std::sync::atomic::{AtomicPtr, Ordering};

// Global reference to AppDelegate for hotkey dispatching
pub(crate) static APP_DELEGATE: AtomicPtr<AppDelegate> = AtomicPtr::new(std::ptr::null_mut());

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

// Helper functions
fn egui_app_from_window(window: &NSWindow) -> Option<RefMut<'_, TrrpyApp>> {
    if let Some(content_view) = window.contentView() {
        let egui_view: &EguiView =
            unsafe { &*((&*content_view) as *const NSView as *const EguiView) };
        if let Some(state) = egui_view.ivars().state.get() {
            return Some(state.app.borrow_mut());
        }
    }
    None
}

fn store_app_pid(window: &NSWindow) {
    unsafe {
        let workspace = NSWorkspace::sharedWorkspace();
        let active_app = workspace.frontmostApplication();
        if let Some(app_obj) = active_app {
            let pid: i32 = msg_send![&*app_obj, processIdentifier];
            if let Some(mut app) = egui_app_from_window(window) {
                app.prev_pid = Some(pid as u32);
                ll(&format!("🔙 Stored previous app PID in TrrpyApp: {}", pid));
            }
        }
    }
}

pub(crate) fn restore_focus(window: &NSWindow) {
    if let Some(app) = egui_app_from_window(window) {
        if let Some(pid) = app.prev_pid {
            let running_app_class =
                AnyClass::get(CStr::from_bytes_with_nul(b"NSRunningApplication\0").unwrap())
                    .unwrap();
            let prev_app: *mut AnyObject = unsafe {
                msg_send![running_app_class, runningApplicationWithProcessIdentifier: pid as i32]
            };
            if !prev_app.is_null() {
                let _: bool = unsafe { msg_send![prev_app, activateWithOptions: 1u64 << 1] };
            }
        }
    }
}

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
                let delegate_ptr = APP_DELEGATE.load(Ordering::SeqCst);
                if !delegate_ptr.is_null() {
                    let delegate = &*delegate_ptr;
                    let selector = sel!(showEguiWindow);
                    let _: () = msg_send![delegate, performSelectorOnMainThread: selector, withObject: std::ptr::null::<NSObject>(), waitUntilDone:false];
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

// Custom NSWindow subclass to allow borderless window to become key/main window
define_class!(
    #[unsafe(super(NSWindow))]
    #[thread_kind = MainThreadOnly]
    #[derive(Debug)]
    pub(crate) struct CustomWindow;

    unsafe impl NSObjectProtocol for CustomWindow {}

    impl CustomWindow {
        #[unsafe(method(canBecomeKeyWindow))]
        fn can_become_key_window(&self) -> bool {
            true
        }
        #[unsafe(method(canBecomeMainWindow))]
        fn can_become_main_window(&self) -> bool {
            true
        }
    }
);

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    pub(crate) struct WindowDelegate;

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
    pub(crate) fn new(mtm: MainThreadMarker) -> objc2::rc::Retained<Self> {
        let this = Self::alloc(mtm);
        let this = this.set_ivars(());
        unsafe { msg_send![super(this), init] }
    }
}

#[derive(Debug)]
pub(crate) struct AppIvars {
    pub(crate) window: Option<Retained<CustomWindow>>,
}

define_class!(
    // SAFETY:
    // - The superclass NSObject does not have any subclassing requirements.
    // - `AppDelegate` does not implement `Drop`.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = AppIvars]
    pub(crate) struct AppDelegate;

    unsafe impl NSObjectProtocol for AppDelegate {}

    unsafe impl NSApplicationDelegate for AppDelegate {
        #[unsafe(method(applicationDidFinishLaunching:))]
        fn did_finish_launching(&self, notification: &NSNotification) {
            ll("🪧 Did finish launching!");
            ll(&format!("🪧 Process ID: {}", std::process::id()));
            // Do something with the notification
            dbg!(notification);

            // Register the global hotkey (Cmd+Shift+K)
            unsafe {
                register_hotkey();
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
                (&*window).orderOut(None);
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
                if (&*window).isVisible() {
                    ll("🙈 Window is visible, hiding it...");
                    (&*window).orderOut(None);
                    restore_focus(&*window);
                    return;
                } else {
                    store_app_pid(&*window);
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
            #[allow(deprecated)]
            app.activateIgnoringOtherApps(true);

            // Create a borderless window for popup-style UI
            let frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(400.0, 300.0));
            let style_mask = NSWindowStyleMask::Borderless;
            let backing_store_type = NSBackingStoreType::Buffered;

            // Allocate and initialize your custom window subclass using Objective-C messaging
            let window: objc2::rc::Retained<CustomWindow> = unsafe {
                let w = CustomWindow::alloc(mtm);
                objc2::msg_send![w, initWithContentRect:frame,
                    styleMask:style_mask,
                    backing:backing_store_type,
                    defer:false]
            };
            // No title for borderless window
            (&*window).center();

            // Set window level to floating to ensure it appears above other apps
            ll("🔝 Setting window level to floating...");
            (&*window).setLevel(3); // NSFloatingWindowLevel = 3

            // Enable mouse moved events for borderless window
            (&*window).setAcceptsMouseMovedEvents(true);

            // Create and set window delegate to handle close events
            let window_delegate = WindowDelegate::new(mtm);
            (&*window).setDelegate(Some(objc2::runtime::ProtocolObject::from_ref(
                &*window_delegate,
            )));

            // Store the delegate to prevent deallocation
            std::mem::forget(window_delegate);

            // Create our custom egui view
            let view = EguiView::new(mtm);

            // Set the view as the window's content view
            (&*window).setContentView(Some(&view));

            // IMPORTANT: Initialize the egui/wgpu state *after* the view is in the window.
            view.init_state();
            // Store the previous frontmost app PID before activating ourselves
            unsafe {
                let workspace = NSWorkspace::sharedWorkspace();
                let active_app = workspace.frontmostApplication();
                if let Some(app_obj) = active_app {
                    let pid: i32 = objc2::msg_send![&*app_obj, processIdentifier]; // TODO check for -1
                    if let Some(state) = view.ivars().state.get() {
                        state.app.borrow_mut().prev_pid = Some(pid as u32);
                        ll(&format!("🔙 Stored previous app PID in TrrpyApp: {}", pid));
                    }
                }
            }

            // Store the window reference for future show/hide operations
            // Safety: We need to get a mutable reference to the ivars
            let ivars_ptr = self.ivars() as *const AppIvars as *mut AppIvars;
            unsafe {
                (*ivars_ptr).window = Some(window.clone());
            }

            // Show and focus the window
            ll("🪟 Making window key and ordering front...");
            (&*window).makeKeyAndOrderFront(None);

            // Ensure the window is at the front and focused
            ll("🔝 Bringing window to front regardless...");
            unsafe {
                (&*window).orderFrontRegardless();
            }

            // Make the view the first responder so it can receive keyboard events immediately
            ll("⌨️ Setting first responder...");
            (&*window).makeFirstResponder(Some(&view));

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
            if let Some(content_view) = (&*window).contentView() {
                (&*window).makeFirstResponder(Some(&content_view));
            }

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
    pub(crate) fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm);
        let this = this.set_ivars(AppIvars { window: None });
        unsafe { msg_send![super(this), init] }
    }
}

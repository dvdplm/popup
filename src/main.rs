//! Implementing `NSApplicationDelegate` for a custom class.
#![allow(unsafe_op_in_unsafe_fn)]
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, ProtocolObject};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy, NSApplicationDelegate};
use objc2_foundation::{
    NSCopying, NSNotification, NSObject, NSObjectProtocol, NSString, ns_string,
};
use std::ffi::c_int;
use std::os::raw::{c_uint, c_void};
use std::sync::atomic::{AtomicPtr, Ordering};

#[link(name = "Foundation", kind = "framework")]
unsafe extern "C" {
    unsafe fn NSLog(format: *const objc2::runtime::AnyObject, ...);
}

fn ll(msg: &str) {
    let ns_msg: Retained<AnyObject> = NSString::from_str(msg).into();
    unsafe {
        NSLog(Retained::as_ptr(&ns_msg));
    }
}

#[derive(Debug)]
#[allow(unused)]
struct Ivars {
    ivar: u8,
    another_ivar: bool,
    box_ivar: Box<i32>,
    maybe_box_ivar: Option<Box<i32>>,
    id_ivar: Retained<NSString>,
    maybe_retained_ivar: Option<Retained<NSString>>,
}

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

static mut EVENT_TAP: *mut c_void = std::ptr::null_mut();

// Global reference to NSApplication for signal handler
static APP_INSTANCE: AtomicPtr<NSApplication> = AtomicPtr::new(std::ptr::null_mut());

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
                ll("✅ HOTKEY PRESSED! You called me master!");

                // Return null to consume the event (prevent it from propagating)
                return std::ptr::null_mut();
            }
        }
    }

    // Return the event unchanged for other keys
    event
}

unsafe fn register_hotkey() {
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
    ll("🪧 Press Cmd+Shift+K to trigger the hotkey");
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
            // Access instance variables
            dbg!(self.ivars());

            // Register the global hotkey (Cmd+Shift+M)
            unsafe {
                register_hotkey();
            }

            // Removed NSApplication::main call - already in main loop
        }

        #[unsafe(method(applicationWillTerminate:))]
        fn will_terminate(&self, _notification: &NSNotification) {
            ll("🪧 Will terminate!");
        }
    }
);

impl AppDelegate {
    fn new(ivar: u8, another_ivar: bool, mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm);
        let this = this.set_ivars(Ivars {
            ivar,
            another_ivar,
            box_ivar: Box::new(2),
            maybe_box_ivar: None,
            id_ivar: NSString::from_str("abc"),
            maybe_retained_ivar: Some(ns_string!("def").copy()),
        });
        unsafe { msg_send![super(this), init] }
    }
}

// Signal handler for SIGINT (Ctrl+C)
extern "C" fn sigint_handler(_signal: c_int) {
    ll("🪧 Received SIGINT (Ctrl+C) - initiating proper app termination...");

    let app_ptr = APP_INSTANCE.load(Ordering::SeqCst);
    if !app_ptr.is_null() {
        unsafe {
            let app = &*app_ptr;
            // Use performSelectorOnMainThread to ensure thread safety
            let selector = objc2::sel!(terminate:);
            let _: () = objc2::msg_send![app, performSelectorOnMainThread: selector, withObject:std::ptr::null::<NSObject>(), waitUntilDone:false];
            ll("✅ Cocoa exit.");
        }
    } else {
        ll("☑ No app instance available, exiting directly");
        std::process::exit(0);
    }
}

unsafe fn setup_signal_handler() {
    unsafe extern "C" {
        fn signal(sig: c_int, handler: extern "C" fn(c_int)) -> extern "C" fn(c_int);
    }

    const SIGINT: c_int = 2;
    unsafe {
        signal(SIGINT, sigint_handler);
    }
    ll("✅ Set up SIGINT handler for graceful Ctrl+C handling");
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
        setup_signal_handler();
    }

    // configure the application delegate
    let delegate = AppDelegate::new(42, true, mtm);
    let object = ProtocolObject::from_ref(&*delegate);
    app.setDelegate(Some(object));

    // run the app
    app.run();
}

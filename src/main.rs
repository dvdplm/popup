//! Implementing `NSApplicationDelegate` for a custom class.
#![deny(unsafe_op_in_unsafe_fn)]
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, ProtocolObject};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy, NSApplicationDelegate};
use objc2_foundation::{
    NSCopying, NSNotification, NSObject, NSObjectProtocol, NSString, ns_string,
};
use std::os::raw::{c_int, c_uint, c_void};

#[link(name = "Foundation", kind = "framework")]
unsafe extern "C" {
    unsafe fn NSLog(format: *const objc2::runtime::AnyObject, ...);
}

fn ll(msg: &str) {
    println!("{}", msg);
    let ns_msg: Retained<AnyObject> = NSString::from_str(msg).into();
    // let ns_msg = NSString::from_str(msg);
    unsafe {
        NSLog(Retained::as_ptr(&ns_msg));
        // NSLog(ns_msg.as_ptr());
        // NSLog(ns_msg.as_ptr());
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

// --- Carbon FFI for global hotkey registration ---

#[link(name = "Carbon", kind = "framework")]
unsafe extern "C" {
    fn RegisterEventHotKey(
        inHotKeyCode: c_uint,
        inHotKeyModifiers: c_uint,
        inHotKeyID: EventHotKeyID,
        inEventTarget: *mut c_void,
        inOptions: c_uint,
        outRef: *mut *mut c_void,
    ) -> c_int;

    fn InstallEventHandler(
        inEventTarget: *mut c_void,
        inHandler: EventHandlerUPP,
        inNumTypes: c_uint,
        inList: *const EventTypeSpec,
        inUserData: *mut c_void,
        outRef: *mut *mut c_void,
    ) -> c_int;
    unsafe fn GetApplicationEventTarget() -> *mut c_void;
}

type EventHandlerUPP = extern "C" fn(*mut c_void, *mut c_void, *mut c_void) -> c_int;

#[repr(C)]
#[derive(Copy, Clone)]
struct EventHotKeyID {
    signature: c_uint,
    id: c_uint,
}

#[repr(C)]
#[derive(Copy, Clone)]
#[allow(non_snake_case)]
struct EventTypeSpec {
    eventClass: c_uint,
    eventKind: c_uint,
}

// Constants for Carbon
const EVENT_CLASS_KEYBOARD: c_uint = 0x6B64776E; // 'kdwn'
const EVENT_HOT_KEY_PRESSED: c_uint = 6;
const CMD_KEY: c_uint = 1 << 8;
const SHIFT_KEY: c_uint = 1 << 9;

// This is the callback for the hotkey event
extern "C" fn hotkey_handler(
    _next_handler: *mut c_void,
    _the_event: *mut c_void,
    _user_data: *mut c_void,
) -> c_int {
    ll("You called me master!");
    0
}

unsafe fn register_hotkey() {
    let hotkey_id = EventHotKeyID {
        signature: 0x1234,
        id: 1,
    };
    let event_type = EventTypeSpec {
        eventClass: EVENT_CLASS_KEYBOARD,
        eventKind: EVENT_HOT_KEY_PRESSED,
    };
    let mut hotkey_ref: *mut c_void = std::ptr::null_mut();
    let event_target: *mut c_void = unsafe { GetApplicationEventTarget() };

    // Keycode for 'M' is 46 on US keyboards
    let keycode_m: c_uint = 46;
    let modifiers = CMD_KEY | SHIFT_KEY;

    let reg_result = unsafe {
        RegisterEventHotKey(
            keycode_m,
            modifiers,
            hotkey_id,
            event_target,
            0,
            &mut hotkey_ref,
        )
    };
    ll(&format!("RegisterEventHotKey result: {}", reg_result));

    let install_result = unsafe {
        InstallEventHandler(
            event_target,
            hotkey_handler,
            1,
            &event_type,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    ll(&format!("InstallEventHandler result: {}", install_result));
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
            ll("Did finish launching!");
            ll(&format!("Process ID: {}", std::process::id()));
            // Do something with the notification
            dbg!(notification);
            // Access instance variables
            dbg!(self.ivars());

            // Register the global hotkey (Cmd+Shift+M)
            unsafe {
                register_hotkey();
            }

            NSApplication::main(MainThreadMarker::from(self));
        }

        #[unsafe(method(applicationWillTerminate:))]
        fn will_terminate(&self, _notification: &NSNotification) {
            ll("Will terminate!");
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

fn main() {
    let mtm: MainThreadMarker = MainThreadMarker::new().unwrap();

    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Regular);

    // configure the application delegate
    let delegate = AppDelegate::new(42, true, mtm);
    let object = ProtocolObject::from_ref(&*delegate);
    app.setDelegate(Some(object));

    // run the app
    app.run();
}

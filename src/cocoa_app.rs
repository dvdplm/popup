use cocoa::appkit::{NSApp, NSApplication, NSApplicationActivationPolicy};
use cocoa::base::nil;
use cocoa::foundation::NSAutoreleasePool;

pub fn run() {
    unsafe {
        let _pool = NSAutoreleasePool::new(nil);
        let app = NSApp();
        app.setActivationPolicy_(NSApplicationActivationPolicy::NSApplicationActivationPolicyRegular);
        app.run();
    }
}

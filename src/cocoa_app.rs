use objc2::rc::autoreleasepool;
use objc2::MainThreadMarker;
use objc2_app_kit::{NSApp, NSApplicationActivationPolicy};

#[allow(dead_code)]
pub fn run() {
    autoreleasepool(|_| {
        let mtm = MainThreadMarker::new().expect("main thread");
        let app = NSApp(mtm);
        app.setActivationPolicy(NSApplicationActivationPolicy::Regular);
        app.run();
    });

}

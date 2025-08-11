## popup

Experiment. Investigates using `egui` and `wgpu` with a Cocoa eventloop.

This project targets **macOS only** and relies on the Cocoa framework.
Building on other platforms is not supported.

## What does it do?

- Setup and start an `NSApplication` and assiociated machinery
- When the application is initialized –– `applicationDidFinishLaunching` in Cocoa parlance –– register a system-global hotkey, hard coded to Cmd + Shift + k in this demo. This means setting up a `CGEventTap` and a runloop source and an event callback.
- Shows an `egui` window when the key combo is pressed.
- Subclasses `NSWindow` so we can have a chrome-less Cocoa window and also receive events.
- Agressively steals focus and ensures we draw in front of everything else.
- The `egui` content is drawn with the GPU with `wgpu`.
- As a demo of using async rust with a main-thread-dependent UI, the `egui` app fetches a live feed of lightning strikes across the globe.

### Key findings

- There are likely way better ways of doing this, e.g. with the `hotkey` and `winit` crates.
- The Cocoa APIs are not too insane to work with from rust. I expected worse; kudos to the `objc2` people.
- The macOS APIs are very rich and there are a lot of interesting things one could do when coupling `egui`, efficient GPU rendering and, say, gesture recognition, OS keychain, automation frameworks, accessibility APIs like voice, video and special purpose input devices.

### Further work

- Dynamic UI generation.
- Custom widgets (bouncy/squishy?)
- Better `tokio` architecture.
- Persist state when showing/hiding the UI.
- Persist state across restarts.

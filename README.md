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

### Run it

Check out the code on a Mac, run `cargo run`, press Cmd+Shift+k to show, ESC or hotkey to close. Check to see if lightning is about to strike.

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



<img width="1027" height="846" alt="Screenshot 2025-08-11 at 13 46 21" src="https://github.com/user-attachments/assets/3d9fb42b-a23b-4dbf-b63c-e7bde10c0796" />

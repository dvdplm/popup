use crate::trrpy::TrrpyApp;
use crate::utils::ll;
use eframe::egui::Context;
use egui_wgpu::Renderer;
use objc2::rc::Retained;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::NSView;
use objc2_foundation::NSRect;
use std::fmt::Debug;
use std::sync::OnceLock;

/// This struct will hold the state for our custom egui view.
/// It's stored in an Ivar in the `EguiView` Objective-C object.
struct EguiViewState {
    /// The egui context, which manages all UI state.
    ctx: Context,
    /// The user's application state (the `eframe::App` implementation).
    app: TrrpyApp,
    /// The wgpu renderer for egui.
    renderer: Renderer,
    /// The wgpu surface to render to.
    surface: wgpu::Surface<'static>,
    /// The wgpu device and queue for sending commands to the GPU.
    device: wgpu::Device,
    queue: wgpu::Queue,
    /// The configuration for the wgpu surface.
    surface_config: wgpu::SurfaceConfiguration,
}

impl Debug for EguiViewState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EguiViewState")
            .field("ctx", &self.ctx)
            .field("app", &self.app)
            .field("surface", &self.surface)
            .field("device", &self.device)
            .field("queue", &self.queue)
            .field("surface_config", &self.surface_config)
            .finish()
    }
}

#[derive(Debug)]
pub(crate) struct Ivars {
    // An instance variable (ivar) to hold a pointer to our Rust state.
    // We use a `Box<OnceLock<...>>` to allow for lazy, one-time initialization
    // after the view has been created and added to a window.
    state: Box<OnceLock<EguiViewState>>,
}
define_class!(
    /// A custom `NSView` that is responsible for hosting and rendering an `egui` UI.
    // SAFETY:
    // - The superclass `NSView` has no special subclassing requirements.
    // - `EguiView` does not implement `Drop`.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[derive(Debug)]
    #[ivars = Ivars]
    pub(crate) struct EguiView;

    impl EguiView {
        /// The main drawing method for the view, called by AppKit when the view needs to be redrawn.
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty_rect: NSRect) {
            ll("🎨 EguiView::draw_rect called");
            // We will implement the rendering logic here.
        }

        /// Informs AppKit that this view can become the "first responder,"
        /// which is necessary for it to receive keyboard events.
        #[unsafe(method(acceptsFirstResponder))]
        fn accepts_first_responder(&self) -> bool {
            true
        }
    }
);

impl EguiView {
    /// Creates a new, uninitialized instance of `EguiView`.
    pub(crate) fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm);
        // Initialize the ivar with an empty OnceLock.
        let this = this.set_ivars(Ivars {
            state: Box::new(OnceLock::new()),
        });
        // Call the designated initializer for NSView.
        unsafe { msg_send![super(this), init] }
    }

    /// Initializes the `wgpu` and `egui` state. This must be called after the
    /// view has been added to a window, because we need a window handle to create the
    /// `wgpu` surface.
    pub(crate) fn init_state(&self) {
        ll("🚀 Initializing EguiView state...");

        let Some(window) = self.window() else {
            ll("❌ EguiView must be in a window to initialize state");
            return;
        };

        if self.ivars().state.get().is_some() {
            ll("⚠️ EguiView state is already initialized.");
            return;
        }

        let (width, height) = {
            let frame = self.frame();
            (frame.size.width as u32, frame.size.height as u32)
        };

        // TODO: Implement the wgpu and egui setup logic here.
        // 1. Get raw window handle from the NSView/NSWindow.
        // 2. Create wgpu instance, adapter, device, and queue.
        // 3. Create wgpu surface using the raw window handle.
        // 4. Configure the surface.
        // 5. Create egui::Context and egui_wgpu::Renderer.
        // 6. Create the user app state (TrrpyApp).
        // 7. Store all of this in the `EguiViewState` and set the `OnceLock`.

        ll("✅ EguiView state initialization placeholder complete.");
    }
}

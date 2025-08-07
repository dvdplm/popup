use crate::trrpy::TrrpyApp;
use crate::utils::ll;
use egui::{self, Context};
use egui_wgpu::Renderer;
use egui_wgpu::wgpu::{
    self, SurfaceTargetUnsafe,
    rwh::{
        AppKitDisplayHandle, AppKitWindowHandle, DisplayHandle, HandleError, HasDisplayHandle,
        HasWindowHandle, RawDisplayHandle, RawWindowHandle, WindowHandle,
    },
};
use objc2::rc::Retained;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::NSView;
use objc2_foundation::NSRect;
use std::cell::RefCell;
use std::fmt::Debug;
use std::sync::OnceLock;

/// This struct will hold the state for our custom egui view.
/// It's stored in an Ivar in the `EguiView` Objective-C object.
struct EguiViewState {
    /// The egui context, which manages all UI state.
    ctx: Context,
    /// The user's application state (the `eframe::App` implementation).
    app: RefCell<TrrpyApp>,
    /// The wgpu renderer for egui.
    renderer: RefCell<Renderer>,
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
            let Some(state) = self.ivars().state.get() else {
                ll("❌ EguiView state not initialized, skipping draw.");
                return;
            };

            let output_frame = match state.surface.get_current_texture() {
                Ok(frame) => frame,
                Err(wgpu::SurfaceError::Lost) => {
                    // Reconfigure the surface if it's lost.
                    ll("⚠️ wgpu surface lost, reconfiguring...");
                    state.surface.configure(&state.device, &state.surface_config);
                    return;
                }
                Err(e) => {
                    ll(&format!("❌ Failed to acquire next swap chain texture: {}", e));
                    return;
                }
            };

            let output_view = output_frame
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default());

            // For now, we'll create empty input. Later, we'll populate this from NSEvents.
            let raw_input = egui::RawInput::default();

            let full_output = state.ctx.run(raw_input, |ctx| {
                state.app.borrow_mut().update(ctx);
            });

            let clipped_primitives = state
                .ctx
                .tessellate(full_output.shapes, full_output.pixels_per_point);

            let screen_descriptor = egui_wgpu::ScreenDescriptor {
                size_in_pixels: [state.surface_config.width, state.surface_config.height],
                pixels_per_point: full_output.pixels_per_point,
            };

            let mut encoder =
                state
                    .device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("egui_command_encoder"),
                    });

            // Upload all resources to the GPU.
            for (id, image_delta) in &full_output.textures_delta.set {
                state
                    .renderer.borrow_mut()
                    .update_texture(&state.device, &state.queue, *id, image_delta);
            }
            for id in &full_output.textures_delta.free {
                state.renderer.borrow_mut().free_texture(id);
            }

            // Upload all resources to the GPU.
            for (id, image_delta) in &full_output.textures_delta.set {
                state
                    .renderer
                    .borrow_mut()
                    .update_texture(&state.device, &state.queue, *id, image_delta);
            }
            for id in &full_output.textures_delta.free {
                state.renderer.borrow_mut().free_texture(id);
            }

            let mut renderer = state.renderer.borrow_mut();
            renderer.update_buffers(
                &state.device,
                &state.queue,
                &mut encoder,
                &clipped_primitives,
                &screen_descriptor,
            );
            // Record all render passes.
            {
                let render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("egui_render_pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &output_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });
                let mut render_pass = render_pass.forget_lifetime();
                renderer.render(&mut render_pass, &clipped_primitives, &screen_descriptor);
            }


            // Submit the command buffer.
            state.queue.submit(Some(encoder.finish()));
            output_frame.present();

            let repaint_delay = full_output.viewport_output.get(&egui::ViewportId::ROOT).map_or(std::time::Duration::from_secs(10), |vo| vo.repaint_delay);
            if repaint_delay.is_zero() {
                unsafe {self.setNeedsDisplay(true)};
            }
        }

        /// Informs AppKit that this view can become the "first responder,"
        /// which is necessary for it to receive keyboard events.
        #[unsafe(method(acceptsFirstResponder))]
        fn accepts_first_responder(&self) -> bool {
            true
        }
    }
);

/// By implementing `HasWindowHandle` for our custom view, we can pass it
/// directly to `wgpu`'s `create_surface` method (for wgpu v0.25).
impl HasWindowHandle for EguiView {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        let view_ptr = self as *const _ as *mut std::ffi::c_void;
        let view_ptr = std::ptr::NonNull::new(view_ptr).ok_or(HandleError::Unavailable)?;
        let wh = AppKitWindowHandle::new(view_ptr);
        let raw = RawWindowHandle::AppKit(wh);
        Ok(unsafe { WindowHandle::borrow_raw(raw) })
    }
}

impl HasDisplayHandle for EguiView {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        let dh = AppKitDisplayHandle::new();
        let raw = RawDisplayHandle::AppKit(dh);
        Ok(unsafe { DisplayHandle::borrow_raw(raw) })
    }
}

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

        let Some(_window) = self.window() else {
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

        // 1. Create wgpu instance and surface.
        // Because we have implemented `HasRawWindowHandle` for `EguiView`,
        // we can pass `self` directly to `create_surface`.
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let target = unsafe { SurfaceTargetUnsafe::from_window(self).unwrap() };
        let surface = unsafe { instance.create_surface_unsafe(target).unwrap() };

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .expect("Failed to find a suitable wgpu adapter");

        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("egui_wgpu_device"),
            required_features: wgpu::Features::default(),
            required_limits: wgpu::Limits::default(),
            ..Default::default()
        }))
        .expect("Failed to create wgpu device");

        // 3. Configure the surface
        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width,
            height,
            present_mode: surface_caps.present_modes[0],
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &surface_config);

        // 4. Create egui context and renderer
        let ctx = Context::default();
        let renderer = Renderer::new(&device, surface_format, None, 1, false);

        // 5. Create the user app state and wrap fields in RefCell
        let app = RefCell::new(TrrpyApp::default());
        let renderer = RefCell::new(renderer);

        // 6. Store the state
        let state = EguiViewState {
            ctx,
            app,
            renderer,
            surface,
            device,
            queue,
            surface_config,
        };

        if self.ivars().state.set(state).is_err() {
            ll("❌ Failed to set EguiView state because it was already set.");
        } else {
            ll("✅ EguiView state initialized and set successfully.");
            // Request a redraw now that we are initialized.
            unsafe { self.setNeedsDisplay(true) };
        }
    }
}

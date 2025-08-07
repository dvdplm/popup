use crate::trrpy::TrrpyApp;
use crate::utils::ll;
use egui::ViewportInfo;
use egui::{self, Context, Event, Key, Modifiers, PointerButton, Pos2, RawInput, Vec2};
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
use objc2_foundation::{NSPoint, NSRect};
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::OnceLock;
use std::time::{Instant, SystemTime};

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
    /// Event handling
    events: RefCell<Vec<Event>>,
    last_frame_time: RefCell<Instant>,
    mouse_pos: RefCell<Pos2>,
    modifiers: RefCell<Modifiers>,
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
            .field("events", &self.events)
            .field("mouse_pos", &self.mouse_pos)
            .field("modifiers", &self.modifiers)
            .finish()
    }
}

impl EguiViewState {
    /// Convert NSPoint to egui coordinates (flip Y axis and account for view bounds)
    fn ns_point_to_egui_pos(&self, ns_point: NSPoint, view_height: f64) -> Pos2 {
        Pos2::new(ns_point.x as f32, (view_height - ns_point.y) as f32)
    }

    /// Convert NSEvent keycode to egui Key
    fn ns_keycode_to_egui_key(&self, keycode: u16) -> Option<Key> {
        match keycode {
            // Letters
            0 => Some(Key::A),
            1 => Some(Key::S),
            2 => Some(Key::D),
            3 => Some(Key::F),
            4 => Some(Key::H),
            5 => Some(Key::G),
            6 => Some(Key::Z),
            7 => Some(Key::X),
            8 => Some(Key::C),
            9 => Some(Key::V),
            11 => Some(Key::B),
            12 => Some(Key::Q),
            13 => Some(Key::W),
            14 => Some(Key::E),
            15 => Some(Key::R),
            17 => Some(Key::T),
            16 => Some(Key::Y),
            32 => Some(Key::U),
            34 => Some(Key::I),
            31 => Some(Key::O),
            35 => Some(Key::P),
            45 => Some(Key::N),
            46 => Some(Key::M),

            // Numbers
            18 => Some(Key::Num1),
            19 => Some(Key::Num2),
            20 => Some(Key::Num3),
            21 => Some(Key::Num4),
            23 => Some(Key::Num5),
            22 => Some(Key::Num6),
            26 => Some(Key::Num7),
            28 => Some(Key::Num8),
            25 => Some(Key::Num9),
            29 => Some(Key::Num0),

            // Special keys
            36 => Some(Key::Enter),
            48 => Some(Key::Tab),
            49 => Some(Key::Space),
            51 => Some(Key::Backspace),
            53 => Some(Key::Escape),

            // Arrow keys
            123 => Some(Key::ArrowLeft),
            124 => Some(Key::ArrowRight),
            125 => Some(Key::ArrowDown),
            126 => Some(Key::ArrowUp),

            _ => None,
        }
    }

    /// Convert NSEvent modifier flags to egui Modifiers
    fn ns_modifiers_to_egui(&self, ns_flags: u64) -> Modifiers {
        Modifiers {
            alt: (ns_flags & 0x080000) != 0,     // NSEventModifierFlagOption
            ctrl: (ns_flags & 0x040000) != 0,    // NSEventModifierFlagControl
            shift: (ns_flags & 0x020000) != 0,   // NSEventModifierFlagShift
            mac_cmd: (ns_flags & 0x100000) != 0, // NSEventModifierFlagCommand
            command: (ns_flags & 0x100000) != 0, // Use Cmd as the main command key on macOS
        }
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

            // Collect accumulated events and prepare input for egui
            let mut events = state.events.borrow_mut();
            let now = Instant::now();
            let last_frame_time = *state.last_frame_time.borrow();
            let frame_time = now.duration_since(last_frame_time);
            *state.last_frame_time.borrow_mut() = now;

            let mut viewports = HashMap::default();
            viewports.insert(egui::ViewportId::ROOT, ViewportInfo {
                native_pixels_per_point: Some(1.0),
                monitor_size: Some(Vec2::new(1920.0, 1080.0)),
                inner_rect: Some(egui::Rect::from_min_size(
                    Pos2::ZERO,
                    Vec2::new(state.surface_config.width as f32, state.surface_config.height as f32)
                )),
                outer_rect: Some(egui::Rect::from_min_size(
                    Pos2::ZERO,
                    Vec2::new(state.surface_config.width as f32, state.surface_config.height as f32)
                )),
                ..Default::default()
            });

            let raw_input = RawInput {
                viewport_id: egui::ViewportId::ROOT,
                viewports,
                screen_rect: Some(egui::Rect::from_min_size(
                    Pos2::ZERO,
                    Vec2::new(state.surface_config.width as f32, state.surface_config.height as f32)
                )),
                max_texture_side: Some(2048),
                time: Some(SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs_f64()),
                predicted_dt: frame_time.as_secs_f32(),
                modifiers: *state.modifiers.borrow(),
                events: events.drain(..).collect(),
                hovered_files: Vec::new(),
                dropped_files: Vec::new(),
                focused: true,
                system_theme: None,
            };

            // Run egui and update app state
                let full_output = state.ctx.run(raw_input, |ctx| {
                    state.app.borrow_mut().update(ctx);
                });

                // Hide window if ESC was pressed
                if state.app.borrow().esc_pressed {
                    if let Some(window) = self.window() {
                        window.orderOut(None);
                    }
                    state.app.borrow_mut().esc_pressed = false; // Reset flag
                    return;
                }

            // Handle viewport commands (like window close)
            for (viewport_id, viewport_output) in &full_output.viewport_output {
                if *viewport_id == egui::ViewportId::ROOT {
                    for command in &viewport_output.commands {
                        match command {
                            egui::ViewportCommand::Close => {
                                ll("🚪 Egui requested window close - hiding window...");
                                // Hide the window instead of terminating
                                if let Some(window) = self.window() {
                                    window.orderOut(None);
                                }
                                return;
                            }
                            _ => {}
                        }
                    }
                }
            }

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

        /// Handle mouse down events
        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, event: *mut objc2::runtime::AnyObject) {
            if let Some(state) = self.ivars().state.get() {
                let location: NSPoint = unsafe { objc2::msg_send![event, locationInWindow] };
                let local_point = self.convertPoint_fromView(location, None);
                let view_height = self.frame().size.height;

                let pos = state.ns_point_to_egui_pos(local_point, view_height);
                *state.mouse_pos.borrow_mut() = pos;

                state.events.borrow_mut().push(egui::Event::PointerButton {
                    pos,
                    button: PointerButton::Primary,
                    pressed: true,
                    modifiers: *state.modifiers.borrow(),
                });

                unsafe { self.setNeedsDisplay(true) };
            }
        }

        /// Handle mouse up events
        #[unsafe(method(mouseUp:))]
        fn mouse_up(&self, event: *mut objc2::runtime::AnyObject) {
            if let Some(state) = self.ivars().state.get() {
                let location: NSPoint = unsafe { objc2::msg_send![event, locationInWindow] };
                let local_point = self.convertPoint_fromView(location, None);
                let view_height = self.frame().size.height;

                let pos = state.ns_point_to_egui_pos(local_point, view_height);
                *state.mouse_pos.borrow_mut() = pos;

                state.events.borrow_mut().push(egui::Event::PointerButton {
                    pos,
                    button: PointerButton::Primary,
                    pressed: false,
                    modifiers: *state.modifiers.borrow(),
                });

                unsafe { self.setNeedsDisplay(true) };
            }
        }

        /// Handle mouse moved events
        #[unsafe(method(mouseMoved:))]
        fn mouse_moved(&self, event: *mut objc2::runtime::AnyObject) {
            if let Some(state) = self.ivars().state.get() {
                let location: NSPoint = unsafe { objc2::msg_send![event, locationInWindow] };
                let local_point = self.convertPoint_fromView(location, None);
                let view_height = self.frame().size.height;

                let pos = state.ns_point_to_egui_pos(local_point, view_height);
                *state.mouse_pos.borrow_mut() = pos;

                state.events.borrow_mut().push(egui::Event::PointerMoved(pos));

                unsafe { self.setNeedsDisplay(true) };
            }
        }

        /// Handle mouse dragged events
        #[unsafe(method(mouseDragged:))]
        fn mouse_dragged(&self, event: *mut objc2::runtime::AnyObject) {
            if let Some(state) = self.ivars().state.get() {
                let location: NSPoint = unsafe { objc2::msg_send![event, locationInWindow] };
                let local_point = self.convertPoint_fromView(location, None);
                let view_height = self.frame().size.height;

                let pos = state.ns_point_to_egui_pos(local_point, view_height);
                *state.mouse_pos.borrow_mut() = pos;

                state.events.borrow_mut().push(egui::Event::PointerMoved(pos));

                unsafe { self.setNeedsDisplay(true) };
            }
        }

        /// Handle key down events
        #[unsafe(method(keyDown:))]
        fn key_down(&self, event: *mut objc2::runtime::AnyObject) {
            let mut handled = false;
            if let Some(state) = self.ivars().state.get() {
                let keycode: u16 = unsafe { objc2::msg_send![event, keyCode] };
                let modifier_flags: u64 = unsafe { objc2::msg_send![event, modifierFlags] };

                let modifiers = state.ns_modifiers_to_egui(modifier_flags);
                *state.modifiers.borrow_mut() = modifiers;

                if let Some(key) = state.ns_keycode_to_egui_key(keycode) {
                    state.events.borrow_mut().push(egui::Event::Key {
                        key,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers,
                    });
                    handled = true;
                }

                // Handle text input
                let characters: *mut objc2::runtime::AnyObject = unsafe { objc2::msg_send![event, characters] };
                if !characters.is_null() {
                    let length: usize = unsafe { objc2::msg_send![characters, length] };
                    if length > 0 {
                        for i in 0..length {
                            let ch: u16 = unsafe { objc2::msg_send![characters, characterAtIndex: i] };
                            if let Some(unicode_char) = char::from_u32(ch as u32) {
                                if unicode_char.is_control() {
                                    continue;
                                }
                                state.events.borrow_mut().push(egui::Event::Text(unicode_char.to_string()));
                                handled = true;
                            }
                        }
                    }
                }

                unsafe { self.setNeedsDisplay(true) };
            }
            if !handled {
                unsafe { objc2::msg_send![super(self), keyDown: event] }
            }
        }

        /// Handle key up events
        #[unsafe(method(keyUp:))]
        fn key_up(&self, event: *mut objc2::runtime::AnyObject) {
            let mut handled = false;
            if let Some(state) = self.ivars().state.get() {
                let keycode: u16 = unsafe { objc2::msg_send![event, keyCode] };
                let modifier_flags: u64 = unsafe { objc2::msg_send![event, modifierFlags] };

                let modifiers = state.ns_modifiers_to_egui(modifier_flags);
                *state.modifiers.borrow_mut() = modifiers;

                if let Some(key) = state.ns_keycode_to_egui_key(keycode) {
                    state.events.borrow_mut().push(egui::Event::Key {
                        key,
                        physical_key: None,
                        pressed: false,
                        repeat: false,
                        modifiers,
                    });
                    handled = true;
                }

                unsafe { self.setNeedsDisplay(true) };
            }
            if !handled {
                unsafe { objc2::msg_send![super(self), keyUp: event] }
            }
        }

        /// Handle modifier key changes
        #[unsafe(method(flagsChanged:))]
        fn flags_changed(&self, event: *mut objc2::runtime::AnyObject) {
            if let Some(state) = self.ivars().state.get() {
                let modifier_flags: u64 = unsafe { objc2::msg_send![event, modifierFlags] };
                let modifiers = state.ns_modifiers_to_egui(modifier_flags);
                *state.modifiers.borrow_mut() = modifiers;

                unsafe { self.setNeedsDisplay(true) };
            }
        }

        /// Handle scroll wheel events
        #[unsafe(method(scrollWheel:))]
        fn scroll_wheel(&self, event: *mut objc2::runtime::AnyObject) {
            if let Some(state) = self.ivars().state.get() {
                let delta_x: f64 = unsafe { objc2::msg_send![event, scrollingDeltaX] };
                let delta_y: f64 = unsafe { objc2::msg_send![event, scrollingDeltaY] };

                // Convert to egui's scroll delta (positive means scroll up/right)
                let delta = Vec2::new(delta_x as f32, delta_y as f32);

                state.events.borrow_mut().push(egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta,
                    modifiers: *state.modifiers.borrow(),
                });

                unsafe { self.setNeedsDisplay(true) };
            }
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
            events: RefCell::new(Vec::new()),
            last_frame_time: RefCell::new(Instant::now()),
            mouse_pos: RefCell::new(Pos2::ZERO),
            modifiers: RefCell::new(Modifiers::default()),
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

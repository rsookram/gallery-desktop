use std::{env::args_os, ffi::CString, num::NonZeroU32};

use gl::types::GLint;
use glutin::{
    config::{ConfigTemplateBuilder, GlConfig},
    context::{ContextAttributesBuilder, PossiblyCurrentContext},
    display::{GetGlDisplay, GlDisplay},
    prelude::{GlSurface, NotCurrentGlContext},
    surface::{Surface as GlutinSurface, SurfaceAttributesBuilder, WindowSurface},
};
use glutin_winit::DisplayBuilder;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use skia_safe::{
    Color, ColorType, Surface,
    gpu::{self, SurfaceOrigin, backend_render_targets, gl::FramebufferInfo},
};
use state::State;
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalPosition,
    event::{ElementState, KeyEvent, Modifiers, MouseButton, WindowEvent},
    event_loop::EventLoop,
    keyboard::{Key, NamedKey},
    window::{Window, WindowAttributes},
};

mod file_container;
mod selector;
mod state;
mod viewer;

fn main() {
    let mut args = args_os();
    args.next();

    let args = args.collect::<Vec<_>>();
    if args.is_empty() {
        eprintln!("no files provided");
        return;
    }

    let el = EventLoop::new().expect("Failed to create event loop");

    let window_attributes = WindowAttributes::default()
        .with_decorations(false)
        .with_fullscreen(Some(winit::window::Fullscreen::Borderless(None)));

    let template = ConfigTemplateBuilder::new();

    let display_builder = DisplayBuilder::new().with_window_attributes(window_attributes.into());
    let (window, gl_config) = display_builder
        .build(&el, template, |configs| {
            // Find the config with the minimum number of samples. Usually Skia takes care of
            // anti-aliasing and may not be able to create appropriate Surfaces for samples > 0.
            // See https://github.com/rust-skia/rust-skia/issues/782
            // And https://github.com/rust-skia/rust-skia/issues/764
            configs
                .reduce(|accum, config| {
                    let transparency_check = config.supports_transparency().unwrap_or(false)
                        & !accum.supports_transparency().unwrap_or(false);

                    if transparency_check || config.num_samples() < accum.num_samples() {
                        config
                    } else {
                        accum
                    }
                })
                .unwrap()
        })
        .unwrap();

    let window = window.expect("Could not create window with OpenGL context");
    let raw_window_handle = RawWindowHandle::from(
        window
            .window_handle()
            .expect("Failed to retrieve WindowHandle"),
    );

    let context_attributes = ContextAttributesBuilder::new().build(None);

    let not_current_gl_context = unsafe {
        gl_config
            .display()
            .create_context(&gl_config, &context_attributes)
            .expect("failed to create context")
    };

    let (width, height): (u32, u32) = window.inner_size().into();

    let attrs = SurfaceAttributesBuilder::<WindowSurface>::new().build(
        raw_window_handle,
        NonZeroU32::new(width).unwrap(),
        NonZeroU32::new(height).unwrap(),
    );

    let gl_surface = unsafe {
        gl_config
            .display()
            .create_window_surface(&gl_config, &attrs)
            .expect("Could not create gl window surface")
    };

    let gl_context = not_current_gl_context
        .make_current(&gl_surface)
        .expect("Could not make GL context current when setting up skia renderer");

    gl::load_with(|s| {
        gl_config
            .display()
            .get_proc_address(CString::new(s).unwrap().as_c_str())
    });
    let interface = skia_safe::gpu::gl::Interface::new_load_with(|name| {
        if name == "eglGetCurrentDisplay" {
            return std::ptr::null();
        }
        gl_config
            .display()
            .get_proc_address(CString::new(name).unwrap().as_c_str())
    })
    .expect("Could not create interface");

    let mut gr_context = skia_safe::gpu::direct_contexts::make_gl(interface, None)
        .expect("Could not create direct context");

    let fb_info = {
        let mut fboid: GLint = 0;
        unsafe { gl::GetIntegerv(gl::FRAMEBUFFER_BINDING, &mut fboid) };

        FramebufferInfo {
            fboid: fboid.try_into().unwrap(),
            format: skia_safe::gpu::gl::Format::RGBA8.into(),
            ..Default::default()
        }
    };

    fn create_surface(
        window: &Window,
        fb_info: FramebufferInfo,
        gr_context: &mut skia_safe::gpu::DirectContext,
        num_samples: usize,
        stencil_size: usize,
    ) -> Surface {
        let size = window.inner_size();
        let size = (
            size.width.try_into().expect("Could not convert width"),
            size.height.try_into().expect("Could not convert height"),
        );
        let backend_render_target =
            backend_render_targets::make_gl(size, num_samples, stencil_size, fb_info);

        gpu::surfaces::wrap_backend_render_target(
            gr_context,
            &backend_render_target,
            SurfaceOrigin::BottomLeft,
            ColorType::RGBA8888,
            None,
            None,
        )
        .expect("Could not create skia surface")
    }

    let num_samples = gl_config.num_samples() as usize;
    let stencil_size = gl_config.stencil_size() as usize;

    let surface = create_surface(&window, fb_info, &mut gr_context, num_samples, stencil_size);

    // Guarantee the drop order inside the FnMut closure. `Window` _must_ be dropped after
    // `DirectContext`.
    //
    // <https://github.com/rust-skia/rust-skia/issues/476>
    struct Env {
        surface: Surface,
        gl_surface: GlutinSurface<WindowSurface>,
        gr_context: skia_safe::gpu::DirectContext,
        gl_context: PossiblyCurrentContext,
        window: Window,
    }

    let env = Env {
        surface,
        gl_surface,
        gl_context,
        gr_context,
        window,
    };

    struct Application {
        env: Env,
        fb_info: FramebufferInfo,
        num_samples: usize,
        stencil_size: usize,
        modifiers: Modifiers,
        mouse_position: PhysicalPosition<f64>,
        state: State,
    }

    let mut application = Application {
        env,
        fb_info,
        num_samples,
        stencil_size,
        modifiers: Modifiers::default(),
        mouse_position: PhysicalPosition { x: 0.0, y: 0.0 },
        state: State::new(args),
    };

    impl ApplicationHandler for Application {
        fn resumed(&mut self, _event_loop: &winit::event_loop::ActiveEventLoop) {}

        fn new_events(
            &mut self,
            _event_loop: &winit::event_loop::ActiveEventLoop,
            _cause: winit::event::StartCause,
        ) {
        }

        fn window_event(
            &mut self,
            event_loop: &winit::event_loop::ActiveEventLoop,
            _window_id: winit::window::WindowId,
            event: WindowEvent,
        ) {
            let mut draw_frame = false;

            match event {
                WindowEvent::CloseRequested => {
                    event_loop.exit();
                    return;
                }
                WindowEvent::Resized(physical_size) => {
                    self.env.surface = create_surface(
                        &self.env.window,
                        self.fb_info,
                        &mut self.env.gr_context,
                        self.num_samples,
                        self.stencil_size,
                    );
                    // First resize the opengl drawable
                    let (width, height): (u32, u32) = physical_size.into();

                    self.env.gl_surface.resize(
                        &self.env.gl_context,
                        NonZeroU32::new(width.max(1)).unwrap(),
                        NonZeroU32::new(height.max(1)).unwrap(),
                    );

                    self.state.width = i32::try_from(width).unwrap();
                    self.state.height = i32::try_from(height).unwrap();
                }
                WindowEvent::ModifiersChanged(new_modifiers) => self.modifiers = new_modifiers,
                WindowEvent::KeyboardInput {
                    event:
                        KeyEvent {
                            logical_key, state, ..
                        },
                    ..
                } => {
                    if self.modifiers.state().super_key() && logical_key == "q" {
                        event_loop.exit();
                    }
                    if !state.is_pressed() {
                        return;
                    }

                    match &mut self.state.screen {
                        state::Screen::Selector(_) => {
                            if logical_key != Key::Named(NamedKey::Enter) {
                                return;
                            }

                            self.state.move_to_viewer();
                        }
                        state::Screen::Viewer(screen) => {
                            if logical_key == "j" {
                                screen.next_image();
                            } else if logical_key == "k" {
                                screen.previous_image();
                            } else if logical_key == "l" {
                                screen.previous_file();
                            } else if logical_key == "h" {
                                screen.next_file();
                            } else if logical_key == "p" {
                                screen.toggle_progress_display();
                            } else {
                                return;
                            }
                        }
                    }

                    self.env.window.request_redraw();
                }
                WindowEvent::CursorMoved { position, .. } => {
                    self.mouse_position = position;
                }
                WindowEvent::MouseInput { state, button, .. } => {
                    if state != ElementState::Pressed {
                        return;
                    }
                    let state::Screen::Selector(screen) = &mut self.state.screen else {
                        return;
                    };

                    match button {
                        MouseButton::Left => {
                            let PhysicalPosition { x, y } = self.mouse_position;
                            screen.on_click(x, y, self.state.width, self.state.height);
                        }
                        MouseButton::Back => {
                            screen.previous_page();
                        }
                        MouseButton::Forward => {
                            screen.next_page();
                        }
                        _ => return,
                    }

                    self.env.window.request_redraw();
                }
                WindowEvent::RedrawRequested => {
                    draw_frame = true;
                }
                _ => (),
            }

            if draw_frame {
                let canvas = self.env.surface.canvas();
                canvas.clear(Color::BLACK);

                match &mut self.state.screen {
                    state::Screen::Selector(screen) => {
                        selector::render_frame(self.state.width, self.state.height, screen, canvas)
                    }
                    state::Screen::Viewer(screen) => {
                        viewer::render_frame(self.state.width, self.state.height, screen, canvas);
                    }
                }
                self.env.gr_context.flush_and_submit();
                self.env
                    .gl_surface
                    .swap_buffers(&self.env.gl_context)
                    .unwrap();
            }
        }
    }

    el.run_app(&mut application).expect("run() failed");
}

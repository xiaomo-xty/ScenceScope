#![cfg_attr(not(debug_assertions), deny(warnings))]

//! Native desktop entry point for `SceneScope`.

use std::sync::Arc;

use anyhow::Context;
use scenescope_core::MeshInstance;
use scenescope_render::GpuState;
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalPosition,
    event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{self, ControlFlow, EventLoop},
    window::Window,
};

// #[derive(Default)]
struct App {
    mesh: MeshInstance,
    window: Option<Arc<Window>>,
    gpu: Option<GpuState>,
    orbit_gesture: OrbitGesture,
    fatal_error: Option<anyhow::Error>,
}

impl ApplicationHandler for App {
    fn window_event(
        &mut self,
        event_loop: &event_loop::ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        let Some(window) = self.window.clone() else {
            return;
        };

        // Only handle events for the window we created.
        if window.id() != window_id {
            return;
        }

        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                if let Err(err) = self.render() {
                    self.fatal_error = Some(err);
                    event_loop.exit();
                    return;
                }
                // do something to redraw the window
                window.request_redraw();
            }
            WindowEvent::Resized(size) => {
                // if size.width == 0 || size.height == 0 {
                //     // Ignore zero-sized windows, as they cannot be rendered to.
                //     return;
                // }

                if let Some(gpu) = self.gpu.as_mut() {
                    gpu.resize(size);

                    if size.width > 0 && size.height > 0 {
                        window.request_redraw();
                    }
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                self.handle_mouse_input(state, button);
            }

            WindowEvent::CursorMoved { position, .. } => {
                self.handle_cursor_moved(position, &window);
            }

            WindowEvent::MouseWheel { delta, .. } => {
                self.handle_mouse_wheel(delta, &window);
            }
            WindowEvent::Focused(false) | WindowEvent::CursorLeft { .. } => {
                self.orbit_gesture = OrbitGesture::Inactive;
            }
            _ => {}
        }
    }

    /// Because the method behaves differently on OS's event driven model,
    /// the method maybe called multiple times,
    /// so we should implement it as a idempotent function.
    fn resumed(&mut self, event_loop: &event_loop::ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let window = event_loop
            .create_window(Window::default_attributes().with_title("SceneScope"))
            .context("failed to create the desktop window");

        match window {
            Ok(window) => {
                let window = Arc::new(window);

                let gpu_result = pollster::block_on(GpuState::new(Arc::clone(&window), &self.mesh));

                match gpu_result {
                    Ok(gpu) => {
                        // request a redraw to trigger the first frame rendering
                        window.request_redraw();

                        self.window = Some(window);
                        self.gpu = Some(gpu);
                    }
                    Err(err) => {
                        self.fatal_error = Some(err);
                        event_loop.exit();
                    }
                }
            }
            Err(err) => {
                self.fatal_error = Some(err);
                event_loop.exit();
            }
        }
    }
}

impl App {
    const fn new(mesh: MeshInstance) -> Self {
        Self {
            mesh,
            window: None,
            gpu: None,
            orbit_gesture: OrbitGesture::Inactive,
            fatal_error: None,
        }
    }
    // fn fatal_error(&self) -> Option<&anyhow::Error> {
    //     self.fatal_error.as_ref()
    // }

    fn render(&mut self) -> anyhow::Result<()> {
        let Some(gpu) = self.gpu.as_mut() else {
            anyhow::bail!("GPU state is not initialized");
        };
        gpu.render()
    }

    fn handle_mouse_input(&mut self, state: ElementState, button: MouseButton) {
        if button != MouseButton::Left {
            return;
        }

        self.orbit_gesture = match state {
            ElementState::Pressed => OrbitGesture::Armed,
            ElementState::Released => OrbitGesture::Inactive,
        }
    }

    fn handle_cursor_moved(&mut self, position: PhysicalPosition<f64>, window: &Window) {
        let position = position.cast::<f32>();

        match self.orbit_gesture {
            OrbitGesture::Inactive => {}
            OrbitGesture::Armed => {
                self.orbit_gesture = OrbitGesture::Dragging { previous: position }
            }
            OrbitGesture::Dragging { previous } => {
                let delta_x = position.x - previous.x;
                let delta_y = position.y - previous.y;

                self.orbit_gesture = OrbitGesture::Dragging { previous: position };

                if let Some(gpu) = self.gpu.as_mut() {
                    gpu.orbit_camera(
                        -delta_x * YAW_RADIANS_PER_PIXEL,
                        -delta_y * PITCH_RADIANS_PER_PIXEL,
                    );

                    window.request_redraw();
                }
            }
        }
    }

    fn handle_mouse_wheel(&mut self, delta: MouseScrollDelta, window: &Window) {
        let zoom_delta = match delta {
            MouseScrollDelta::LineDelta(_, y) => y,
            MouseScrollDelta::PixelDelta(position) => position.cast::<f32>().y,
        };

        if let Some(gpu) = self.gpu.as_mut() {
            gpu.zoom_camera(zoom_delta);
            window.request_redraw();
        }
    }
}

// Runs the native SceneScope desktop application.
///
/// # Errors
///
/// Returns an error if the event loop cannot be created or run, or if
/// window or GPU initialization fails.
pub fn run() -> anyhow::Result<()> {
    let event_loop = EventLoop::new().context("failed to create the desktop event loop")?;

    // let mut app = App::default();
    let mesh =
        scenescope_gltf::parse_first_mesh_primitive(include_bytes!("../../../assets/test/Box.glb"))
            .context("failed to parse the embedded Box.glb")?;

    let mut app = App::new(mesh);

    // using poll mode to avoid blocking the event loop, which is important for real-time applications
    event_loop.set_control_flow(ControlFlow::Poll);
    event_loop
        .run_app::<App>(&mut app)
        .context("failed to run the desktop event loop")?;

    if let Some(error) = app.fatal_error.take() {
        return Err(error);
    }
    Ok(())
}

// fn main() -> anyhow::Result<()> {
//     run()
// }

// const ORBIT_RADIANS_PER_PIXEL: f32 = 0.005;

const YAW_RADIANS_PER_PIXEL: f32 = 0.005;
const PITCH_RADIANS_PER_PIXEL: f32 = -0.005;

// const PIXELS_PER_SCROLL_LINE: f32 = 100.0;

enum OrbitGesture {
    Inactive,
    Armed,
    Dragging { previous: PhysicalPosition<f32> },
}

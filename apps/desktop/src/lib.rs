#![cfg_attr(not(debug_assertions), deny(warnings))]

//! Native desktop entry point for `SceneScope`.

use std::{sync::Arc, thread};

use anyhow::Context;
use scenescope_core::asset::AssetDocument;
use scenescope_render::GpuState;
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalPosition,
    event::{ElementState, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{self, ControlFlow, EventLoop, EventLoopProxy},
    keyboard::Key,
    window::Window,
};

// #[derive(Default)]
struct App {
    document: AssetDocument,
    proxy: EventLoopProxy<UserEvent>,
    window: Option<Arc<Window>>,
    gpu: Option<GpuState>,
    orbit_gesture: OrbitGesture,
    fatal_error: Option<anyhow::Error>,
}

impl ApplicationHandler<UserEvent> for App {
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
            WindowEvent::KeyboardInput { event, .. } => {
                self.handle_key_input(&event);
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

    fn user_event(&mut self, _event_loop: &event_loop::ActiveEventLoop, event: UserEvent) {
        match event {
            UserEvent::AssetPicked(pick) => self.handle_asset_pick(pick),
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

                let instances = self.document.mesh_instances();
                // Temporary single-instance rendering: M2-004 renders every instance.
                let Some(instance) = instances.first().copied() else {
                    self.fatal_error = Some(anyhow::anyhow!(
                        "asset contains no renderable mesh instances"
                    ));
                    event_loop.exit();
                    return;
                };

                let gpu_result = pollster::block_on(GpuState::new(Arc::clone(&window), instance));

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
    const fn new(document: AssetDocument, proxy: EventLoopProxy<UserEvent>) -> Self {
        Self {
            document,
            proxy,
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

    fn handle_key_input(&self, event: &KeyEvent) {
        if event.state != ElementState::Pressed || event.repeat {
            return;
        }

        let Key::Character(character) = event.logical_key.as_ref() else {
            return;
        };

        if character.eq_ignore_ascii_case("o") {
            self.begin_asset_pick();
        }
    }

    /// Opens the file dialog on a worker thread so the event loop keeps running.
    fn begin_asset_pick(&self) {
        let proxy = self.proxy.clone();

        thread::spawn(move || {
            let outcome = pick_asset();

            if let Err(error) = proxy.send_event(UserEvent::AssetPicked(outcome)) {
                eprintln!("failed to report the picked asset: {error}");
            }
        });
    }

    /// Loads the picked asset, keeping the current one on screen when anything fails.
    ///
    /// Cancelling is not an error: it simply leaves the current asset untouched.
    fn handle_asset_pick(&mut self, pick: AssetPick) {
        let bytes = match pick {
            AssetPick::Picked(bytes) => bytes,
            AssetPick::Cancelled => return,
            AssetPick::Failed(error) => {
                eprintln!("asset pick failed: {error}");
                return;
            }
        };

        let document = match scenescope_gltf::parse_asset_document(&bytes) {
            Ok(document) => document,
            Err(error) => {
                eprintln!("failed to parse the picked asset: {error}");
                return;
            }
        };

        let instances = document.mesh_instances();
        let Some(instance) = instances.first().copied() else {
            eprintln!("the picked asset contains no renderable mesh instance");
            return;
        };

        let Some(gpu) = self.gpu.as_mut() else {
            eprintln!("GPU state is not initialized");
            return;
        };

        if let Err(error) = gpu.set_mesh(instance) {
            eprintln!("failed to upload the picked asset: {error}");
            return;
        }

        self.document = document;
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
        if let Some(gpu) = self.gpu.as_mut() {
            gpu.zoom_camera(scenescope_render::wheel_notches(delta));
            window.request_redraw();
        }
    }
}

/// Runs the file dialog and reads the picked file on the calling thread.
///
/// `FileHandle::read` returns `Vec<u8>` and panics on IO errors, so the file is
/// read through its path to keep the failure visible.
fn pick_asset() -> AssetPick {
    let Some(file) = pollster::block_on(
        rfd::AsyncFileDialog::new()
            .set_title("SceneScope: open a GLB")
            .add_filter("glTF Binary", &["glb"])
            .pick_file(),
    ) else {
        return AssetPick::Cancelled;
    };

    let path = file.path().to_path_buf();

    match std::fs::read(&path) {
        Ok(bytes) => {
            println!("picked {}: {} bytes", file.file_name(), bytes.len());
            AssetPick::Picked(bytes)
        }
        Err(error) => AssetPick::Failed(format!("failed to read {}: {error}", path.display())),
    }
}

/// Runs the native `SceneScope` desktop application.
///
/// # Errors
///
/// Returns an error if the event loop cannot be created or run, or if
/// window or GPU initialization fails.
pub fn run() -> anyhow::Result<()> {
    let event_loop = EventLoop::<UserEvent>::with_user_event()
        .build()
        .context("failed to create the desktop event loop")?;

    let document =
        scenescope_gltf::parse_asset_document(include_bytes!("../../../assets/test/Box.glb"))
            .context("failed to parse the embedded Box.glb")?;

    let proxy = event_loop.create_proxy();
    let mut app = App::new(document, proxy);

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

/// Work completed off the event-loop thread.
enum UserEvent {
    AssetPicked(AssetPick),
}

/// Outcome of one file-dialog interaction.
enum AssetPick {
    /// The user picked a file; its bytes are already read.
    Picked(Vec<u8>),
    /// The user dismissed the dialog.
    Cancelled,
    /// The dialog or the file read failed.
    Failed(String),
}

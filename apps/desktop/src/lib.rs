#![cfg_attr(not(debug_assertions), deny(warnings))]

//! Native desktop entry point for `SceneScope`.

use std::sync::Arc;

use anyhow::Context;
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{self, ControlFlow, EventLoop},
    window::Window,
};

use crate::gpu::GpuState;

pub mod gpu;

#[allow(
    clippy::redundant_pub_crate,
    reason = "Vertex types are crate-visible to the sibling GPU module while this module remains private."
)]
mod vertex;

#[derive(Default)]
struct App {
    window: Option<Arc<Window>>,
    gpu: Option<GpuState>,
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

                let gpu_result = pollster::block_on(GpuState::new(Arc::clone(&window)));

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
    // fn fatal_error(&self) -> Option<&anyhow::Error> {
    //     self.fatal_error.as_ref()
    // }

    fn render(&mut self) -> anyhow::Result<()> {
        let Some(gpu) = self.gpu.as_mut() else {
            anyhow::bail!("GPU state is not initialized");
        };
        gpu.render()
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

    let mut app = App::default();

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

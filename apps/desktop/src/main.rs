#![cfg_attr(not(debug_assertions), deny(warnings))]

//! Native desktop entry point for `SceneScope`.

use anyhow::Context;
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{self, ControlFlow, EventLoop},
    window::Window,
};

#[derive(Default)]
struct App {
    window: Option<Window>,
    fatal_error: Option<anyhow::Error>,
}

impl ApplicationHandler for App {
    fn window_event(
        &mut self,
        event_loop: &event_loop::ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        let Some(window) = self.window.as_ref() else {
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
                // do something to redraw the window
                window.request_redraw();
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
                self.window = Some(window);
            }
            Err(err) => {
                self.fatal_error = Some(err);
                event_loop.exit();
            }
        }
    }
}

fn run() -> anyhow::Result<()> {
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

fn main() -> anyhow::Result<()> {
    run()
}

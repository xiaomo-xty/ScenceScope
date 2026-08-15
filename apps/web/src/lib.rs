//! Browser entry point for `SceneScope`.
#[cfg(target_arch = "wasm32")]
mod browser {
    use std::{fmt::Display, sync::Arc};

    use scenescope_render::GpuState;
    use wasm_bindgen::{JsValue, prelude::wasm_bindgen};
    use winit::{
        application::ApplicationHandler,
        event::WindowEvent,
        event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy},
        platform::web::{EventLoopExtWebSys, WindowAttributesExtWebSys},
        window::{Window, WindowId},
    };

    enum UserEvent {
        GpuInitialized(Result<GpuState, String>),
    }

    enum RendererState {
        WaitingForSize,
        Initializing,
        Ready(Box<GpuState>),
        Failed,
    }

    struct App {
        window: Option<Arc<Window>>,
        proxy: EventLoopProxy<UserEvent>,
        renderer: RendererState,
    }

    impl App {
        const fn new(proxy: EventLoopProxy<UserEvent>) -> Self {
            Self {
                proxy,
                window: None,
                renderer: RendererState::WaitingForSize,
            }
        }

        fn try_begin_gpu_initialization(&mut self) {
            if !matches!(self.renderer, RendererState::WaitingForSize) {
                return;
            }

            let Some(window) = self.window.as_ref().map(Arc::clone) else {
                return;
            };

            let size = window.inner_size();

            if size.width == 0 || size.height == 0 {
                return;
            }

            self.renderer = RendererState::Initializing;

            let proxy = self.proxy.clone();

            // send a initialized signal to UserEvent
            wasm_bindgen_futures::spawn_local(async move {
                let result = initialize_gpu(window).await;

                if let Err(error) = proxy.send_event(UserEvent::GpuInitialized(result)) {
                    web_sys::console::error_1(&js_error(&error));
                }
            });
        }
    }

    async fn initialize_gpu(window: Arc<Window>) -> Result<GpuState, String> {
        let mesh = scenescope_gltf::parse_first_mesh_primitive(include_bytes!(
            "../../../assets/test/Box.glb"
        ))
        .map_err(|error| error.to_string())?;

        GpuState::new(window, &mesh)
            .await
            .map_err(|error| format!("{error:#}"))
    }

    impl ApplicationHandler<UserEvent> for App {
        fn resumed(&mut self, event_loop: &ActiveEventLoop) {
            if self.window.is_some() {
                return;
            }

            let attributes = Window::default_attributes()
                .with_title("SceneScope")
                .with_append(true);

            match event_loop.create_window(attributes) {
                Ok(window) => {
                    self.window = Some(Arc::new(window));
                    self.try_begin_gpu_initialization();
                }
                Err(error) => {
                    web_sys::console::error_1(&js_error(&error));
                    event_loop.exit();
                }
            }
        }

        fn user_event(&mut self, event_loop: &ActiveEventLoop, event: UserEvent) {
            match event {
                UserEvent::GpuInitialized(Ok(mut gpu)) => {
                    let Some(window) = self.window.as_ref() else {
                        self.renderer = RendererState::Failed;
                        event_loop.exit();
                        return;
                    };

                    let size = window.inner_size();
                    gpu.resize(size);

                    self.renderer = RendererState::Ready(Box::new(gpu));

                    if size.width > 0 && size.height > 0 {
                        window.request_redraw();
                    }
                }
                UserEvent::GpuInitialized(Err(error)) => {
                    web_sys::console::error_1(&JsValue::from_str(&error));
                    event_loop.exit();
                }
            }
        }
        fn window_event(
            &mut self,
            event_loop: &ActiveEventLoop,
            window_id: WindowId,
            event: WindowEvent,
        ) {
            let Some(window) = self.window.as_ref() else {
                return;
            };

            if window.id() != window_id {
                return;
            }

            match event {
                WindowEvent::CloseRequested => {
                    event_loop.exit();
                }
                WindowEvent::RedrawRequested => {
                    let RendererState::Ready(gpu) = &mut self.renderer else {
                        return;
                    };

                    if let Err(error) = gpu.render() {
                        web_sys::console::error_1(&js_error(&error));
                        event_loop.exit();
                    }
                }
                WindowEvent::Resized(size) => {
                    if matches!(self.renderer, RendererState::WaitingForSize) {
                        if size.width > 0 && size.height > 0 {
                            self.try_begin_gpu_initialization();
                        }
                        return;
                    }

                    if let RendererState::Ready(gpu) = &mut self.renderer {
                        gpu.resize(size);

                        if size.width > 0 && size.height > 0 {
                            window.request_redraw();
                        }
                    }
                }
                _ => {}
            }
        }
    }

    fn js_error(error: &impl Display) -> JsValue {
        JsValue::from_str(&error.to_string())
    }

    /// Starts the browser application.
    ///
    /// # Errors
    ///
    /// Returns an error if the browser event loop cannot be created.
    #[wasm_bindgen(start)]
    pub fn start() -> Result<(), JsValue> {
        let event_loop = EventLoop::<UserEvent>::with_user_event()
            .build()
            .map_err(|error| js_error(&error))?;

        let proxy = event_loop.create_proxy();
        event_loop.spawn_app(App::new(proxy));

        Ok(())
    }
}

#[cfg(target_arch = "wasm32")]
pub use browser::start;

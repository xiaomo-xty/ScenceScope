#![cfg_attr(not(debug_assertions), deny(warnings))]

//! Browser entry point for `SceneScope`.
#[cfg(target_arch = "wasm32")]
mod browser {
    use std::{fmt::Display, sync::Arc};

    use scenescope_core::asset::AssetDocument;
    use scenescope_render::GpuState;
    use wasm_bindgen::{JsValue, prelude::wasm_bindgen};
    use winit::{
        application::ApplicationHandler,
        dpi::PhysicalPosition,
        event::{ElementState, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent},
        event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy},
        keyboard::Key,
        platform::web::{EventLoopExtWebSys, WindowAttributesExtWebSys},
        window::{Window, WindowId},
    };

    enum UserEvent {
        GpuInitialized(Result<Box<(GpuState, AssetDocument)>, String>),
        AssetPicked(AssetPick),
    }

    /// Outcome of one file-dialog interaction.
    enum AssetPick {
        /// The user picked a file; its bytes have already been read.
        Picked(Vec<u8>),
        /// The user dismissed the dialog.
        Cancelled,
    }

    enum RendererState {
        WaitingForSize,
        Initializing,
        Ready(Box<GpuState>),
        Failed,
    }

    const YAW_RADIANS_PER_PIXEL: f32 = 0.005;
    const PITCH_RADIANS_PER_PIXEL: f32 = -0.005;

    enum OrbitGesture {
        Inactive,
        Armed,
        Dragging { previous: PhysicalPosition<f32> },
    }

    struct App {
        document: Option<AssetDocument>,
        window: Option<Arc<Window>>,
        proxy: EventLoopProxy<UserEvent>,
        renderer: RendererState,
        orbit_gesture: OrbitGesture,
    }

    impl App {
        const fn new(proxy: EventLoopProxy<UserEvent>) -> Self {
            Self {
                document: None,
                proxy,
                window: None,
                renderer: RendererState::WaitingForSize,
                orbit_gesture: OrbitGesture::Inactive,
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
                let result = initialize_gpu(window)
                    .await
                    .map(|(gpu, document)| Box::new((gpu, document)));

                if let Err(error) = proxy.send_event(UserEvent::GpuInitialized(result)) {
                    web_sys::console::error_1(&js_error(&error));
                }
            });
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

        /// Opens the browser file dialog; the async block captures only the proxy.
        fn begin_asset_pick(&self) {
            let proxy = self.proxy.clone();

            wasm_bindgen_futures::spawn_local(async move {
                let picked = rfd::AsyncFileDialog::new()
                    .set_title("SceneScope: open a GLB")
                    .add_filter("glTF Binary", &["glb"])
                    .pick_file()
                    .await;

                let outcome = match picked {
                    Some(file) => AssetPick::Picked(file.read().await),
                    None => AssetPick::Cancelled,
                };

                if let Err(error) = proxy.send_event(UserEvent::AssetPicked(outcome)) {
                    web_sys::console::error_1(&js_error(&error));
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
            };

            let document = match scenescope_gltf::parse_asset_document(&bytes) {
                Ok(document) => document,
                Err(error) => {
                    web_sys::console::error_1(&js_error(&format!(
                        "failed to parse the picked asset: {error}"
                    )));
                    return;
                }
            };

            let instances = document.mesh_instances();
            let Some(instance) = instances.first().copied() else {
                web_sys::console::error_1(&JsValue::from_str(
                    "the picked asset contains no renderable mesh instance",
                ));
                return;
            };

            let RendererState::Ready(gpu) = &mut self.renderer else {
                return web_sys::console::error_1(&JsValue::from_str(
                    "GPU state is not initialized",
                ));
            };

            if let Err(error) = gpu.set_mesh(instance) {
                return web_sys::console::error_1(&js_error(&format!(
                    "failed to upload the picked asset: {error}"
                )));
            }

            self.document = Some(document);
        }

        fn handle_mouse_input(&mut self, state: ElementState, button: MouseButton) {
            if button != MouseButton::Left {
                return;
            }

            self.orbit_gesture = match state {
                ElementState::Pressed => OrbitGesture::Armed,
                ElementState::Released => OrbitGesture::Inactive,
            };
        }

        fn handle_cursor_moved(&mut self, position: PhysicalPosition<f64>, window: &Window) {
            let position = position.cast::<f32>();

            match self.orbit_gesture {
                OrbitGesture::Inactive => {}
                OrbitGesture::Armed => {
                    self.orbit_gesture = OrbitGesture::Dragging { previous: position };
                }
                OrbitGesture::Dragging { previous } => {
                    let delta_x = position.x - previous.x;
                    let delta_y = position.y - previous.y;

                    self.orbit_gesture = OrbitGesture::Dragging { previous: position };

                    if let RendererState::Ready(gpu) = &mut self.renderer {
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

            if let RendererState::Ready(gpu) = &mut self.renderer {
                gpu.zoom_camera(zoom_delta);
                window.request_redraw();
            }
        }
    }

    #[expect(
        clippy::future_not_send,
        reason = "wasm is single-threaded and spawn_local drives this future; the surface \
                  holds browser handles that are deliberately not Send"
    )]
    async fn initialize_gpu(window: Arc<Window>) -> Result<(GpuState, AssetDocument), String> {
        let document =
            scenescope_gltf::parse_asset_document(include_bytes!("../../../assets/test/Box.glb"))
                .map_err(|error| error.to_string())?;

        let instances = document.mesh_instances();
        // Temporary single-instance rendering: M2-004 renders every instance.
        let instance = instances
            .first()
            .copied()
            .ok_or_else(|| "asset contains no renderable mesh instance".to_owned())?;

        let gpu = GpuState::new(window, instance)
            .await
            .map_err(|error| format!("{error:#}"))?;

        Ok((gpu, document))
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
                UserEvent::GpuInitialized(Ok(initialized)) => {
                    let (mut gpu, document) = *initialized;
                    let Some(window) = self.window.as_ref() else {
                        self.renderer = RendererState::Failed;
                        event_loop.exit();
                        return;
                    };

                    let size = window.inner_size();
                    gpu.resize(size);

                    self.document = Some(document);
                    self.renderer = RendererState::Ready(Box::new(gpu));

                    if size.width > 0 && size.height > 0 {
                        window.request_redraw();
                    }
                }
                UserEvent::GpuInitialized(Err(error)) => {
                    web_sys::console::error_1(&JsValue::from_str(&error));
                    event_loop.exit();
                }
                UserEvent::AssetPicked(pick) => self.handle_asset_pick(pick),
            }
        }
        fn window_event(
            &mut self,
            event_loop: &ActiveEventLoop,
            window_id: WindowId,
            event: WindowEvent,
        ) {
            let Some(window) = self.window.as_ref().map(Arc::clone) else {
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

                WindowEvent::KeyboardInput { event, .. } => {
                    self.handle_key_input(&event);
                }

                WindowEvent::MouseInput { state, button, .. } => {
                    self.handle_mouse_input(state, button);
                }
                WindowEvent::CursorMoved { position, .. } => {
                    self.handle_cursor_moved(position, window.as_ref());
                }
                WindowEvent::MouseWheel { delta, .. } => {
                    self.handle_mouse_wheel(delta, window.as_ref());
                }
                WindowEvent::Focused(false) | WindowEvent::CursorLeft { .. } => {
                    self.orbit_gesture = OrbitGesture::Inactive;
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

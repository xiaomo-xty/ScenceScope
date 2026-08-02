//! GPU state management for the desktop application.

use std::sync::Arc;

use anyhow::{Context, Ok};
use wgpu::RequestAdapterOptions;
use winit::{dpi::PhysicalSize, window::Window};

#[derive(Debug)]
pub(crate) struct GpuState {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    // pub adapter: wgpu::Adapter,
    pub size: PhysicalSize<u32>,
    pub surface: wgpu::Surface<'static>,
    pub config: wgpu::SurfaceConfiguration,
}

impl GpuState {
    pub(super) async fn new(window: Arc<Window>) -> anyhow::Result<Self> {
        let size = window.inner_size();

        let instance = wgpu::Instance::default();

        let surface = instance.create_surface(window)?;

        let adapter = instance
            .request_adapter(&RequestAdapterOptions {
                compatible_surface: Some(&surface),
                ..Default::default()
            })
            .await
            .context("failed to request a compatibel GPU adapter")?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("SceneScope device"),
                // no dependencies on specific features now,
                // for cross-platform compatibility
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                ..Default::default()
            })
            .await
            .context("failed to request a compatible GPU device")?;

        let config = surface
            .get_default_config(&adapter, size.width, size.height)
            .context("Selected adapter cannot present to the surface")?;

        if size.width == 0 || size.height == 0 {
            anyhow::bail!("Window size is zero, cannot create GPU state");
        }

        surface.configure(&device, &config);

        Ok(Self {
            device,
            queue,
            // adapter,
            size,
            surface,
            config,
        })
    }

    pub(super) fn render(&self) -> anyhow::Result<()> {
        if self.size.width == 0 || self.size.height == 0 {
            // Skip rendering if the window size is zero
            return Ok(());
        }

        let (surface_texture, should_reconfigure) = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(texture) => (texture, false),
            wgpu::CurrentSurfaceTexture::Suboptimal(texture) => (texture, true),
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Outdated => {
                self.surface.configure(&self.device, &self.config);
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Lost => {
                anyhow::bail!("GPU surface was lost");
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                anyhow::bail!("GPU surface validation error");
            }
        };

        {
            let view = surface_texture
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default());

            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("SceneScroe clear encoder"),
                });

            {
                let _render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("SceneScope clear render pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color {
                                r: 0.88,
                                g: 0.10,
                                b: 0.12,
                                a: 1.0,
                            }),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    ..Default::default()
                });
            }
            self.queue.submit([encoder.finish()]);
        }

        self.queue.present(surface_texture);

        if should_reconfigure {
            self.surface.configure(&self.device, &self.config);
        }

        Ok(())
    }
}

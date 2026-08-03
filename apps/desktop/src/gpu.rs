//! GPU state management for the desktop application.

use std::sync::Arc;

use anyhow::{Context, Ok};
use wgpu::{
    PipelineCompilationOptions, PipelineLayoutDescriptor, RenderPipelineDescriptor,
    RequestAdapterOptions, ShaderModuleDescriptor, VertexState, util::DeviceExt,
};
use winit::{dpi::PhysicalSize, window::Window};

use crate::vertex::{TRIANGLE_VERTICES, Vertex};

#[derive(Debug)]
pub(crate) struct GpuState {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    // pub adapter: wgpu::Adapter,
    pub size: PhysicalSize<u32>,
    pub surface: wgpu::Surface<'static>,
    pub config: wgpu::SurfaceConfiguration,

    render_pipeline: wgpu::RenderPipeline,

    vertex_buffer: wgpu::Buffer,
    vertex_count: u32,
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

        let shader_model = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("SceneScope shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("./shaders/hello.wgsl").into()),
        });

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("SceneScope triangle vertex buffer"),
            contents: bytemuck::cast_slice(&TRIANGLE_VERTICES),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let vertex_count =
            u32::try_from(TRIANGLE_VERTICES.len()).context("triangle vertex count exceeds u32")?;

        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("pipeline layout"),
            bind_group_layouts: &[
                // no need now
            ],
            immediate_size: 0,
        });

        let render_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("ScenceScope render pipeline"),
            layout: Some(&pipeline_layout),
            vertex: VertexState {
                module: &shader_model,
                entry_point: Some("vs_main"),
                compilation_options: PipelineCompilationOptions::default(),
                buffers: &[Some(Vertex::layout())],
            },

            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                ..Default::default()
            },

            fragment: Some(wgpu::FragmentState {
                module: &shader_model,
                entry_point: Some("fs_main"),
                compilation_options: PipelineCompilationOptions::default(),
                targets: &[Some(config.format.into())],
            }),

            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        Ok(Self {
            device,
            queue,
            size,
            surface,
            config,
            render_pipeline,

            vertex_buffer,
            vertex_count,
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
                let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
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

                render_pass.set_pipeline(&self.render_pipeline);

                render_pass.set_vertex_buffer(
                    // the slot response to  buffers of VertexState in `render_pipeline`
                    0,
                    self.vertex_buffer.slice(..),
                );

                render_pass.draw(0..self.vertex_count, 0..1);
            }
            self.queue.submit([encoder.finish()]);
        }

        self.queue.present(surface_texture);

        if should_reconfigure {
            self.surface.configure(&self.device, &self.config);
        }

        Ok(())
    }

    /// resize the GPU state and reconfigure the surface
    pub(super) fn resize(&mut self, size: PhysicalSize<u32>) {
        self.size = size;

        // `wgpu::Surface::configure()` don't allow zero width or height,
        // so we should skip the configuration if the size is zero.
        if size.width == 0 || size.height == 0 {
            return;
        }

        self.config.width = size.width;
        self.config.height = size.height;
        self.surface.configure(&self.device, &self.config);
    }
}

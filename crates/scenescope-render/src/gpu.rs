//! GPU state management for the desktop application.

use std::sync::Arc;

use anyhow::{Context, Ok};
use glam::{
    Mat4, Vec3,
    camera::rh::{proj::directx::perspective, view::look_at_mat4},
};
use scenescope_core::{MeshData, MeshInstance};
use wgpu::{
    PipelineCompilationOptions, PipelineLayoutDescriptor, RenderPipelineDescriptor,
    RequestAdapterOptions, ShaderModuleDescriptor, VertexState, util::DeviceExt,
};
use winit::{dpi::PhysicalSize, window::Window};

use crate::vertex::Vertex;

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct CameraUniform {
    view_projection: [[f32; 4]; 4],
}

impl CameraUniform {
    fn new(aspect_ratio: f32) -> Self {
        let view = look_at_mat4(Vec3::new(1.5, 1.5, 2.5), Vec3::ZERO, Vec3::Y);

        let projection = perspective(45.0_f32.to_radians(), aspect_ratio, 0.1, 100.0);

        Self {
            view_projection: (projection * view).to_cols_array_2d(),
        }
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct ObjectUniform {
    model: [[f32; 4]; 4],
    normal_matrix: [[f32; 4]; 4],
}

impl ObjectUniform {
    fn new(world_transform: &[[f32; 4]; 4]) -> anyhow::Result<Self> {
        let model = Mat4::from_cols_array_2d(world_transform);

        let normal_matrix = model
            .try_inverse()
            .context("mesh world transform is not invertiable")?
            .transpose();

        Ok(Self {
            model: model.to_cols_array_2d(),
            normal_matrix: normal_matrix.to_cols_array_2d(),
        })
    }
}

#[allow(
    clippy::cast_precision_loss,
    reason = "window dimensions only need approximate precision for aspect ratio"
)]
const fn aspect_ratio(size: PhysicalSize<u32>) -> f32 {
    size.width as f32 / size.height as f32
}

fn vertices_from_mesh(mesh: &MeshData) -> anyhow::Result<Vec<Vertex>> {
    let Some(normals) = mesh.normals.as_deref() else {
        return Ok(mesh
            .positions
            .iter()
            .copied()
            .map(Vertex::from_position)
            .collect());
    };

    if mesh.positions.len() != normals.len() {
        anyhow::bail!("mesh position and normal counts do not match");
    }

    Ok(mesh
        .positions
        .iter()
        .copied()
        .zip(normals.iter().copied())
        .map(|(position, normal)| Vertex::from_position_and_normal(position, normal))
        .collect())
}

const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth24Plus;

fn create_depth_view(device: &wgpu::Device, size: PhysicalSize<u32>) -> wgpu::TextureView {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("SceneScope depth texture"),
        size: wgpu::Extent3d {
            width: size.width,
            height: size.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: DEPTH_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });

    texture.create_view(&wgpu::TextureViewDescriptor::default())
}

fn create_render_pipeline(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    surface_format: wgpu::TextureFormat,
    camera_layout: &wgpu::BindGroupLayout,
    object_layout: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: Some("pipeline layout"),
        bind_group_layouts: &[Some(camera_layout), Some(object_layout)],
        immediate_size: 0,
    });

    device.create_render_pipeline(&RenderPipelineDescriptor {
        label: Some("ScenceScope render pipeline"),
        layout: Some(&pipeline_layout),
        vertex: VertexState {
            module: shader,
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
            module: shader,
            entry_point: Some("fs_main"),
            compilation_options: PipelineCompilationOptions::default(),
            targets: &[Some(surface_format.into())],
        }),

        depth_stencil: Some(wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: Some(true),
            depth_compare: Some(wgpu::CompareFunction::Less),
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    })
}

fn create_object_binding(
    device: &wgpu::Device,
    world_transform: &[[f32; 4]; 4],
) -> anyhow::Result<(wgpu::BindGroupLayout, wgpu::BindGroup)> {
    // =============================== obejct uniform ===========================================
    let object_uniform = ObjectUniform::new(world_transform)?;

    let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("SceneScop object uniform buffer"),
        contents: bytemuck::bytes_of(&object_uniform),
        usage: wgpu::BufferUsages::UNIFORM,
    });

    let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("SceneScope object bind group layout"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    });

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("SceneScope obejct bind group"),
        layout: &layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: buffer.as_entire_binding(),
        }],
    });

    Ok((layout, bind_group))
}

/// Owns the GPU resources used to render one mesh instance to a window surface
#[derive(Debug)]
pub struct GpuState {
    device: wgpu::Device,
    queue: wgpu::Queue,
    // pub adapter: wgpu::Adapter,
    size: PhysicalSize<u32>,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,

    render_pipeline: wgpu::RenderPipeline,

    vertex_buffer: wgpu::Buffer,
    // vertex_count: u32,
    index_buffer: wgpu::Buffer,
    index_count: u32,

    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,

    // object_buffer: wgpu::Buffer,
    object_bind_group: wgpu::BindGroup,

    depth_view: wgpu::TextureView,
}

impl GpuState {
    /// Creates GPU resources for a window and mesh instance.
    ///
    /// The window must have non-zero physical dimensions. Mesh geometry and
    /// the world transform are uploaded during initialization.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    ///
    /// - the window cannot be used to create a rendering surface;
    /// - no compatible GPU adapter or device is available;
    /// - position and normal counts do not match.
    /// - the mesh world transform is not invertible.
    /// - the index count cannot be represented as `u32`.
    pub async fn new(window: Arc<Window>, mesh_instance: &MeshInstance) -> anyhow::Result<Self> {
        let size = window.inner_size();

        if size.width == 0 || size.height == 0 {
            anyhow::bail!("Window size is zero, cannot create GPU state");
        }

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

        surface.configure(&device, &config);

        let shader_model = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("SceneScope shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("./shaders/hello.wgsl").into()),
        });

        let vertices: Vec<Vertex> = vertices_from_mesh(&mesh_instance.mesh)?;

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("SceneScope vertex buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("SceneScope index buffer"),
            contents: bytemuck::cast_slice(&mesh_instance.mesh.indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        // ======================================== camera_unfiform ==================================

        let camera_uniform = CameraUniform::new(aspect_ratio(size));

        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("SceneScope camera uniform buffer"),
            contents: bytemuck::bytes_of(&camera_uniform),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let camera_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("SceneScope camera bind group layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("SceneScope camera bind group"),
            layout: &camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
        });

        let (object_bind_group_layout, object_bind_group) =
            create_object_binding(&device, &mesh_instance.world_transform)?;

        // let vertex_count =
        //     u32::try_from(QUAD_VERTICES.len()).context("triangle vertex count exceeds u32")?;
        let index_count = u32::try_from(mesh_instance.mesh.indices.len())
            .context("triangle vertex count exceeds u32")?;

        let render_pipeline = create_render_pipeline(
            &device,
            &shader_model,
            config.format,
            &camera_bind_group_layout,
            &object_bind_group_layout,
        );

        let depth_view = create_depth_view(&device, size);

        Ok(Self {
            device,
            queue,
            size,
            surface,
            config,
            render_pipeline,

            vertex_buffer,
            // vertex_count,
            index_buffer,
            index_count,

            camera_buffer,
            camera_bind_group,

            // object_buffer,
            object_bind_group,

            depth_view,
        })
    }

    /// Renders and presents one frame.
    ///
    /// Rendering is skipped while window has zero area or while the surface
    /// is temporarily unavailable.
    ///
    /// # Errors
    ///
    /// Returns an error if the GPU surface is lost or reports a validation error.
    pub fn render(&self) -> anyhow::Result<()> {
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

                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &self.depth_view,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Clear(1.0),
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    }),
                    ..Default::default()
                });

                render_pass.set_pipeline(&self.render_pipeline);

                render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
                render_pass.set_bind_group(1, &self.object_bind_group, &[]);

                render_pass.set_vertex_buffer(
                    // the slot response to  buffers of VertexState in `render_pipeline`
                    0,
                    self.vertex_buffer.slice(..),
                );

                render_pass
                    .set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);

                // render_pass.draw(0..self.vertex_count,  0..1);

                render_pass.draw_indexed(0..self.index_count, 0, 0..1);
            }
            self.queue.submit([encoder.finish()]);
        }

        self.queue.present(surface_texture);

        if should_reconfigure {
            self.surface.configure(&self.device, &self.config);
        }

        Ok(())
    }

    /// Updates the surface configuration and camera projection for a new size.
    ///
    /// A zero-sized window is recorded, but surface reconfiguration is deferred
    /// until a non-zero size is received.
    pub fn resize(&mut self, size: PhysicalSize<u32>) {
        self.size = size;

        // `wgpu::Surface::configure()` don't allow zero width or height,
        // so we should skip the configuration if the size is zero.
        if size.width == 0 || size.height == 0 {
            return;
        }

        let camera_uniform = CameraUniform::new(aspect_ratio(size));

        self.queue
            .write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(&camera_uniform));

        self.config.width = size.width;
        self.config.height = size.height;
        self.surface.configure(&self.device, &self.config);

        self.depth_view = create_depth_view(&self.device, size);
    }
}

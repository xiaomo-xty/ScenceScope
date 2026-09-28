//! GPU state management for the desktop application.

use std::sync::Arc;

use anyhow::{Context, Ok};
use glam::Mat4;
use scenescope_core::MeshInstance;
use wgpu::{
    PipelineCompilationOptions, PipelineLayoutDescriptor, RenderPipelineDescriptor,
    RequestAdapterOptions, ShaderModuleDescriptor, VertexState, util::DeviceExt,
};
use winit::{dpi::PhysicalSize, window::Window};

use crate::{
    camera::{CameraBinding, OrbitCamera, aspect_ratio},
    mesh::MeshBuffers,
    vertex::Vertex,
};

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
    surface_view_format: wgpu::TextureFormat,
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
            cull_mode: Some(wgpu::Face::Back),
            polygon_mode: wgpu::PolygonMode::Fill,
            ..Default::default()
        },

        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some("fs_main"),
            compilation_options: PipelineCompilationOptions::default(),
            targets: &[Some(surface_view_format.into())],
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

/// Owns the GPU resources used to render one mesh instance to a window surface
#[derive(Debug)]
pub struct GpuState {
    device: wgpu::Device,
    queue: wgpu::Queue,
    // pub adapter: wgpu::Adapter,
    surface_state: SurfaceState,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    surface_view_format: wgpu::TextureFormat,

    render_pipeline: wgpu::RenderPipeline,

    mesh_buffers: MeshBuffers,

    camera: OrbitCamera,
    camera_binding: CameraBinding,

    object_binding: ObjectBinding,

    depth_view: wgpu::TextureView,
    // config: wgpu::SurfaceConfiguration,
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

        let mut config = surface
            .get_default_config(&adapter, size.width, size.height)
            .context("Selected adapter cannot present to the surface")?;

        let surface_view_format = configure_srgb_surface_view(&mut config)?;

        surface.configure(&device, &config);

        let shader_model = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("SceneScope shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("./shaders/hello.wgsl").into()),
        });

        let camera = OrbitCamera::new(aspect_ratio(size));
        let camera_binding = CameraBinding::new(&device, &camera);

        let object_binding = ObjectBinding::new(&device, &mesh_instance.world_transform)?;

        let render_pipeline = create_render_pipeline(
            &device,
            &shader_model,
            surface_view_format,
            camera_binding.layout(),
            object_binding.layout(),
        );

        let depth_view = create_depth_view(&device, size);

        let surface_state = SurfaceState::Configured;

        let mesh_buffers = MeshBuffers::new(&device, mesh_instance)?;

        Ok(Self {
            device,
            queue,
            surface_state,
            surface,
            config,
            surface_view_format,
            render_pipeline,

            mesh_buffers,

            camera,
            camera_binding,

            // object_buffer,
            object_binding,

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
    pub fn render(&mut self) -> anyhow::Result<()> {
        match self.surface_state {
            SurfaceState::Suspended => return Ok(()),
            SurfaceState::Configured => {}
            SurfaceState::ResizePending(size) => {
                self.reconfigure_surface(size);
            }
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
                .create_view(&wgpu::TextureViewDescriptor {
                    label: Some("SceneScope sRGB surface view"),
                    format: Some(self.surface_view_format),
                    ..wgpu::TextureViewDescriptor::default()
                });

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

                render_pass.set_bind_group(0, self.camera_binding.bind_group(), &[]);
                render_pass.set_bind_group(1, &self.object_binding.bind_group, &[]);

                render_pass.set_vertex_buffer(
                    // the slot response to  buffers of VertexState in `render_pipeline`
                    0,
                    self.mesh_buffers.vertex_buffer.slice(..),
                );

                render_pass.set_index_buffer(
                    self.mesh_buffers.index_buffer.slice(..),
                    wgpu::IndexFormat::Uint32,
                );

                // render_pass.draw(0..self.vertex_count,  0..1);

                render_pass.draw_indexed(0..self.mesh_buffers.index_count, 0, 0..1);
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
    pub const fn resize(&mut self, size: PhysicalSize<u32>) {
        if size.width == 0 || size.height == 0 {
            self.surface_state = SurfaceState::Suspended;
        } else if size.width == self.config.width && size.height == self.config.height {
            self.surface_state = SurfaceState::Configured;
        } else {
            self.surface_state = SurfaceState::ResizePending(size);
        }
    }

    fn reconfigure_surface(&mut self, size: PhysicalSize<u32>) {
        self.camera.set_aspect_ratio(aspect_ratio(size));

        self.camera_binding
            .update_uniform(&self.queue, &self.camera);

        self.config.width = size.width;
        self.config.height = size.height;

        self.surface.configure(&self.device, &self.config);
        self.depth_view = create_depth_view(&self.device, size);

        self.surface_state = SurfaceState::Configured;
    }

    /// Rotates the orbit camera around its current target.
    ///
    /// `delta_yaw_radians` rotates the camera horizontally around
    /// the world Y axis. A positive value moves the camera from the
    /// positive Z direction toward the positive X direction.
    ///
    /// `delta_pitch_radians` rotates the camera vertically. A positive
    /// value moves the camera above its target. The resulting pitch is
    /// clamped to prevent the view direction from becoming parallel to
    /// the camera's up vector.
    ///
    /// The updated view-projection matrix is written to the camera
    /// uniform buffer immediately.
    pub fn orbit_camera(&mut self, delta_yaw_radians: f32, delta_pitch_radians: f32) {
        self.camera.orbit(delta_yaw_radians, delta_pitch_radians);

        self.camera_binding
            .update_uniform(&self.queue, &self.camera);
    }

    /// Changes the orbit camera's distance from its target.
    ///
    /// A positive `delta` moves the camera closer to the target, while
    /// a negative value moves it farther away. The distance is clamped
    /// to the camera's supported minimum and maximum values.
    ///
    /// The updated view-projection matrix is written to the camera
    /// uniform buffer immediately.
    pub fn zoom_camera(&mut self, delta: f32) {
        self.camera.zoom(delta);

        self.camera_binding
            .update_uniform(&self.queue, &self.camera);
    }

    /// Replaces the currently rendered mesh instance.
    ///
    /// New vertex and index buffers are uploaded for the mesh geometry, and the
    /// object uniform (model and normal matrices) is written to its buffer
    /// immediately. The render pipeline, surface configuration, and orbit camera
    /// are left unchanged; the caller controls framing for the new model.
    ///
    /// Replacement is atomic: every fallible step runs before any state is
    /// swapped, so a failed call leaves the previously loaded mesh on screen.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    ///
    /// - position and normal counts do not match;
    /// - the mesh world transform is not invertible;
    /// - the index count cannot be represented as `u32`.
    pub fn set_mesh(&mut self, mesh_instance: &MeshInstance) -> anyhow::Result<()> {
        let new_buffers = MeshBuffers::new(&self.device, mesh_instance)?;
        self.object_binding
            .update(&self.queue, &mesh_instance.world_transform)?;
        self.mesh_buffers = new_buffers;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SurfaceState {
    Configured,
    ResizePending(PhysicalSize<u32>),
    Suspended,
}

fn configure_srgb_surface_view(
    config: &mut wgpu::SurfaceConfiguration,
) -> anyhow::Result<wgpu::TextureFormat> {
    let view_format = config.format.add_srgb_suffix();

    if !view_format.is_srgb() {
        anyhow::bail!(
            "surface format {:?} has no sRGB-compatible view format",
            config.format,
        );
    }

    config.color_space = wgpu::SurfaceColorSpace::Srgb;

    if view_format != config.format {
        config.view_formats.push(view_format);
    }

    Ok(view_format)
}

#[derive(Debug)]
struct ObjectBinding {
    layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
    uniform_buffer: wgpu::Buffer,
}

impl ObjectBinding {
    fn new(device: &wgpu::Device, world_transform: &[[f32; 4]; 4]) -> anyhow::Result<Self> {
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

        Ok(Self {
            layout,
            bind_group,
            uniform_buffer: buffer,
        })
    }

    fn update(&self, queue: &wgpu::Queue, world_transform: &[[f32; 4]; 4]) -> anyhow::Result<()> {
        let uniform = ObjectUniform::new(world_transform)?; // inveribale matrix -> Err
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&uniform));
        Ok(())
    }

    const fn layout(&self) -> &wgpu::BindGroupLayout {
        &self.layout
    }
}

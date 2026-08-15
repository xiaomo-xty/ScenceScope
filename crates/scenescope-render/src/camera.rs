//! Camera mode

use glam::{
    Vec3,
    camera::rh::{proj::directx::perspective, view::look_at_mat4},
};
use wgpu::util::DeviceExt;
use winit::dpi::PhysicalSize;

// #[derive(Debug)]
// pub(crate) struct OrbitCamera {
//     /// the center of orbit, and the camera's eye point to it always.
//     target: Vec3,
//     yaw: f32,
//     pitch: f32,
//     distance: f32,
//     aspect_ratio: f32,
// }

// impl OrbitCamera {
//     pub(crate) fn eye(&self) -> Vec3 {
//         let horizontal_distance =
//             self.distance * self.pitch.cos();

//         let offset = Vec3::new(
//             horizontal_distance * self.yaw.sin(),
//             self.distance * self.pitch.sin(),
//             horizontal_distance * self.yaw.cos(),
//         );

//         self.target + offset
//     }

//     pub(crate) fn view(&self) -> Mat4 {
//         look_at_mat4(
//             self.eye(),
//             self.target,
//             Vec3::Y
//         )
//     }

//     pub(crate) fn projection(&self) -> Mat4 {
//         perspective(
//             45.0_f32.to_radians(),
//             self.aspect_ratio,
//             0.1,
//             100.0,
//         )
//     }

//     pub(crate) fn view_projection(&self) -> Mat4 {
//         self.projection() * self.view()
//     }

//     pub(crate) fn orbit(&mut self, delta_yaw: f32, delta_pitch: f32) {
//         self.yaw += delta_yaw;

//         const LIMIT: f32 =
//             std::f32::consts::FRAC_PI_2 - 0.01;

//         self.pitch =
//             (self.pitch + delta_pitch).clamp(-LIMIT, LIMIT);
//     }

//     pub(crate) fn zoom(&mut self, delta: f32) {
//         self.distance =
//             (self.distance * (-delta * 0.1).exp())
//                 .clamp(0.1, 100.0);
//     }
// }

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct CameraUniform {
    view_projection: [[f32; 4]; 4],
}

impl CameraUniform {
    pub(crate) fn new(aspect_ratio: f32) -> Self {
        let view = look_at_mat4(Vec3::new(1.5, 1.5, 2.5), Vec3::ZERO, Vec3::Y);

        let projection = perspective(45.0_f32.to_radians(), aspect_ratio, 0.1, 100.0);

        Self {
            view_projection: (projection * view).to_cols_array_2d(),
        }
    }
}

#[derive(Debug)]
pub(crate) struct CameraBinding {
    buffer: wgpu::Buffer,
    layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
}

impl CameraBinding {
    pub(crate) fn new(device: &wgpu::Device, size: PhysicalSize<u32>) -> Self {
        // ======================================== camera_uniform ==================================

        let uniform = CameraUniform::new(aspect_ratio(size));

        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("SceneScope camera uniform buffer"),
            contents: bytemuck::bytes_of(&uniform),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("SceneScope camera bind group"),
            layout: &layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        });

        Self {
            buffer,
            layout,
            bind_group,
        }
    }

    pub(crate) const fn layout(&self) -> &wgpu::BindGroupLayout {
        &self.layout
    }

    pub(crate) const fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }

    pub(crate) const fn buffer(&self) -> &wgpu::Buffer {
        &self.buffer
    }

    // pub(crate) fn update(
    //     &self,
    //     queue: &wgpu::Queue,
    //     camera: &OrbitCamera,
    // ) {
    //     todo!();
    // }
}

#[allow(
    clippy::cast_precision_loss,
    reason = "window dimensions only need approximate precision for aspect ratio"
)]
pub(crate) const fn aspect_ratio(size: PhysicalSize<u32>) -> f32 {
    size.width as f32 / size.height as f32
}

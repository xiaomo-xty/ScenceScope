//! GPU buffers for an uploaded mesh instance.

use anyhow::Context;
use scenescope_core::{MeshData, MeshInstance};
use wgpu::util::DeviceExt;

use crate::vertex::Vertex;

#[derive(Debug)]
// mesh.rs —— mod mesh 本身是私有模块
pub(crate) struct MeshBuffers {
    pub(crate) vertex_buffer: wgpu::Buffer,
    pub(crate) index_buffer: wgpu::Buffer,
    pub(crate) index_count: u32,
}
impl MeshBuffers {
    pub(crate) fn new(device: &wgpu::Device, mesh_instance: &MeshInstance) -> anyhow::Result<Self> {
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

        // let vertex_count =
        //     u32::try_from(QUAD_VERTICES.len()).context("triangle vertex count exceeds u32")?;
        let index_count = u32::try_from(mesh_instance.mesh.indices.len())
            .context("triangle vertex count exceeds u32")?;

        Ok(Self {
            vertex_buffer,
            index_buffer,
            index_count,
        })
    }
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

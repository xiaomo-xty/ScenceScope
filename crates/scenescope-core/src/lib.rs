#![cfg_attr(not(debug_assertions), deny(warnings))]

//! `SceneScope` platform-independent core crate.

use crate::asset::{MeshId, NodeId};

pub mod asset;
pub mod statistics;

/// CPU-side geometry extracted from an asset.
#[derive(Clone, Debug, PartialEq)]
pub struct MeshData {
    /// Vertex positions.
    pub positions: Vec<[f32; 3]>,

    /// Triangle vertex indices normalized to `u32`.
    pub indices: Vec<u32>,

    /// Vertex normals supplied by the source asset.
    pub normals: Option<Vec<[f32; 3]>>,

    /// UV coordinates.
    pub uvs: Option<Vec<[f32; 2]>>,
}

/// A mesh primitive placed in world space, borrowed from an asset document.
///
/// Geometry is borrowed from the document; the world transform is computed
/// during scene traversal, so it is owned by this value. The identifiers record
/// which node placed the primitive and which mesh owns it, so an interface can
/// map a scene tree selection onto the instances it should highlight.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MeshInstance<'a> {
    /// Node that places this primitive in the scene.
    pub node_id: NodeId,

    ///  Mesh that owns this primitive.
    pub mesh_id: MeshId,

    /// Index of the primitive within the mesh.
    pub primitive_index: usize,

    /// Geometry of the referenced primitive.
    pub geometry: &'a MeshData,

    /// Column-major transform from local space to world space.
    pub world_transform: [[f32; 4]; 4],
}

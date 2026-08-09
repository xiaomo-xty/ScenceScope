#![cfg_attr(not(debug_assertions), deny(warnings))]

//! `SceneScope` platform-independent core crate.
//!
//! The implementation is intentionally left to the project author.
//!
//!

/// A mesh placed in world space by a scene node.
#[derive(Clone, Debug, PartialEq)]
pub struct MeshInstance {
    /// Shared geometry for this initial single-mesh representation.
    pub mesh: MeshData,

    /// Column-major transform from local space to world space.
    pub world_transform: [[f32; 4]; 4],
}

/// CPU-side geometry extracted from an asset.
#[derive(Clone, Debug, PartialEq)]
pub struct MeshData {
    /// Vertex positions.
    pub positions: Vec<[f32; 3]>,

    /// Triangle vertex indices normalized to `u32`.
    pub indices: Vec<u32>,

    /// Vertex normals supplied by the source asset.
    pub normals: Option<Vec<[f32; 3]>>,
}

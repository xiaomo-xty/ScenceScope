#![cfg_attr(not(debug_assertions), deny(warnings))]

//! `SceneScope` platform-independent core crate.
//!
//! The implementation is intentionally left to the project author.
//!
//!

/// CPU-side geometry extracted from an asset.
#[derive(Clone, Debug, PartialEq)]
pub struct MeshData {
    /// Vertex positions.
    pub positions: Vec<[f32; 3]>,

    /// Triangle vertex indices normalized to `u32`.
    pub indices: Vec<u32>,
}

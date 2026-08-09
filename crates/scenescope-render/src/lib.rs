//! Shared `wgpu` rendering for `SceneScope`.
//!
//! This crate renders platform-independent scene data from
//! `scenescope-core`. Window creation and event-loop management remain
//! the responsibility of platform applications.

mod gpu;

pub use gpu::GpuState;

#[allow(
    clippy::redundant_pub_crate,
    reason = "Vertex types are crate-visible to the sibling GPU module while this module remains private."
)]
mod vertex;

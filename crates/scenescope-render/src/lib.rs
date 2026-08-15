//! Shared `wgpu` rendering for `SceneScope`.
//!
//! This crate renders platform-independent scene data from
//! `scenescope-core`. Window creation and event-loop management remain
//! the responsibility of platform applications.

mod gpu;

#[allow(
    clippy::redundant_pub_crate,
    reason = "Camera types are shared with the sibling GPU module while the camera module remains private."
)]
mod camera;

pub use gpu::GpuState;

#[allow(
    clippy::redundant_pub_crate,
    reason = "Vertex types are crate-visible to the sibling GPU module while this module remains private."
)]
mod vertex;

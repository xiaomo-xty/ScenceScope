#![cfg_attr(not(debug_assertions), deny(warnings))]

//! `glTF` adapter for `SceneScope`.
//!
//!

use glam::Mat4;
use gltf::{Gltf, buffer::Source, mesh::Mode};
use scenescope_core::{MeshData, MeshInstance};
use thiserror::Error;

const GLB_MAGIC: &[u8; 4] = b"glTF";

/// An error produced while parsing a mesh from a binary glTF asset.
#[derive(Debug, Error)]
#[error(transparent)]
pub struct ParseError(#[from] ParseErrorKind);

#[derive(Debug, Error)]
enum ParseErrorKind {
    #[error("input is not a binary glTF file")]
    NotGlb,

    #[error("invalid binary glTF")]
    InvalidGltf(#[source] gltf::Error),

    #[error("external buffers are not supported")]
    ExternalBuffer,

    #[error("binary glTF does not contain a BIN chunk")]
    MissingBinaryBlob,

    #[error("binary glTF does not contain a mesh primitive")]
    MissingMeshPrimitive,

    #[error("first mesh primitive is not a triangle list")]
    UnsupportedPrimitiveMode,

    #[error("first mesh primitive has no readable POSITION attribute")]
    MissingPositions,

    #[error("first mesh primitive has no readable indices")]
    MissingIndices,

    #[error("binary glTF does not contain a scene")]
    MissingScene,

    #[error("scene does not contain a mesh node")]
    MissingMeshNode,
}

fn find_first_mesh_node<'a>(
    node: &gltf::Node<'a>,
    parent_world: Mat4,
) -> Option<(gltf::Mesh<'a>, Mat4)> {
    let local = Mat4::from_cols_array_2d(&node.transform().matrix());

    let world = parent_world * local;

    if let Some(mesh) = node.mesh() {
        return Some((mesh, world));
    }

    node.children()
        .find_map(|child| find_first_mesh_node(&child, world))
}

/// Parses the first mesh primitive from a `GLB` byte slice.
///
/// # Errors
///
/// Returns an error when the input is invalid, requires external buffers,
/// or does not contain a supported indexed triangle primitive.
pub fn parse_first_mesh_primitive(bytes: &[u8]) -> Result<MeshInstance, ParseError> {
    if !bytes.starts_with(GLB_MAGIC) {
        return Err(ParseErrorKind::NotGlb.into());
    }

    let gltf = Gltf::from_slice(bytes).map_err(ParseErrorKind::InvalidGltf)?;

    if gltf
        .buffers()
        .any(|buffer| matches!(buffer.source(), Source::Uri(_)))
    {
        return Err(ParseErrorKind::ExternalBuffer.into());
    }

    // let primitive = gltf
    //     .meshes()
    //     .next()
    //     .and_then(|mesh| mesh.primitives().next())
    //     .ok_or(ParseErrorKind::MissingMeshPrimitive)?;

    let scene = gltf
        .default_scene()
        .or_else(|| gltf.scenes().next())
        .ok_or(ParseErrorKind::MissingScene)?;

    let (mesh, world_transform) = scene
        .nodes()
        .find_map(|node| find_first_mesh_node(&node, Mat4::IDENTITY))
        .ok_or(ParseErrorKind::MissingMeshNode)?;

    let primitive = mesh
        .primitives()
        .next()
        .ok_or(ParseErrorKind::MissingMeshPrimitive)?;

    if primitive.mode() != Mode::Triangles {
        return Err(ParseErrorKind::UnsupportedPrimitiveMode.into());
    }

    let blob = gltf
        .blob
        .as_deref()
        .ok_or(ParseErrorKind::MissingBinaryBlob)?;

    let reader = primitive.reader(|buffer| match buffer.source() {
        Source::Bin => Some(blob),
        Source::Uri(_) => None,
    });

    let positions = reader
        .read_positions()
        .ok_or(ParseErrorKind::MissingPositions)?
        .collect();

    let indices = reader
        .read_indices()
        .ok_or(ParseErrorKind::MissingIndices)?
        .into_u32()
        .collect();

    let normals = reader.read_normals().map(Iterator::collect);

    let mesh = MeshData {
        positions,
        indices,
        normals,
    };

    Ok(MeshInstance {
        mesh,
        world_transform: world_transform.to_cols_array_2d(),
    })
}

#[cfg(test)]
mod tests {
    // use glam::Mat4;

    use super::{ParseError, ParseErrorKind, parse_first_mesh_primitive};

    const BOX_GLB: &[u8] = include_bytes!("../../../assets/test/Box.glb");

    #[test]
    fn parses_box_mesh_primitive() -> Result<(), ParseError> {
        const BOX_WORLD_TRANSFORM: [[f32; 4]; 4] = [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 0.0, -1.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];

        let instance = parse_first_mesh_primitive(BOX_GLB)?;
        let mesh = &instance.mesh;

        assert_eq!(
            mesh.positions.len(),
            24,
            "Box.glb should contain 24 vertex positions"
        );

        assert_eq!(
            mesh.indices.len(),
            36,
            "Box.glb should contain 36 vertex indices"
        );

        let position_count = mesh.positions.len();

        assert!(
            mesh.indices
                .iter()
                .all(|&index| { usize::try_from(index).is_ok_and(|index| index < position_count) }),
            "every index should reference an existing vertex position",
        );

        assert_eq!(
            mesh.normals.as_ref().map(Vec::len),
            Some(mesh.positions.len()),
            "Box.glb should provide one normal per vertex position"
        );

        assert_eq!(
            instance.world_transform, BOX_WORLD_TRANSFORM,
            "Box.glb should preserve its scene-node world transform"
        );

        Ok(())
    }

    #[test]
    fn rejects_non_glb_bytes() {
        let result = parse_first_mesh_primitive(b"not a GLB file");

        assert!(
            matches!(result, Err(ParseError(ParseErrorKind::NotGlb))),
            "non-GLB bytes should produce the NotGlb error"
        );
    }
}

//! Statistics derived from platform-independent asset data.

/// Summary counts for an asset document and its default scene.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AssetStatistics {
    /// Number of scenes stored in the asset document.
    pub scene_count: usize,

    /// Number of nodes stored in the asset document.
    pub node_count: usize,

    /// Number of meshes stored in the asset document.
    pub mesh_count: usize,

    /// Number of mesh primitives stored across all meshes.
    pub primitive_count: usize,

    /// Number of materials stored in the asset document.
    pub material_count: usize,

    /// Number of textures stored in the asset document.
    pub texture_count: usize,

    /// Number of vertices stored across all mesh primitives.
    ///
    /// Shared mesh data is counted once, regardless of how many nodes
    /// instantiate the mesh.
    pub stored_vertex_count: usize,

    /// Number of triangles stored across all mesh primitives.
    ///
    /// Shared mesh data is counted once, regardless of how many nodes
    /// instantiate the mesh.
    pub stored_triangle_count: usize,

    /// Number of valid mesh instances reachable from the default scene.
    ///
    /// Multiple nodes that reference the same mesh are counted separately.
    /// This value is zero when the document has no default scene.
    pub default_scene_mesh_instance_count: usize,

    /// Number of instantiated triangles reachable from the default scene.
    ///
    /// Triangles in a shared mesh are counted once per reachable mesh
    /// instance. This value is zero when the document has no default scene.
    pub default_scene_triangle_count: usize,
}

#[cfg(test)]
mod tests {
    use crate::{
        MeshData,
        asset::{
            AssetDocument, Material, MaterialId, Mesh, MeshId, MeshPrimitive, Node, NodeId,
            NodeTransform, Scene, SceneId, Texture, TextureId,
        },
    };

    use super::AssetStatistics;

    #[test]
    fn counts_stored_resources_once() {
        let document = AssetDocument {
            scenes: vec![Scene {
                name: Some("Main scene".to_owned()),
                roots: vec![NodeId::from_index(0)],
            }],
            default_scene: None,
            nodes: vec![Node {
                name: Some("Mesh node".to_owned()),
                children: Vec::new(),
                mesh: Some(MeshId::from_index(0)),
                transform: NodeTransform::IDENTITY,
            }],
            meshes: vec![Mesh {
                name: Some("Two primitive mesh".to_owned()),
                primitives: vec![
                    MeshPrimitive {
                        geometry: MeshData {
                            positions: vec![[0.0; 3]; 3],
                            indices: vec![0, 1, 2],
                            normals: None,
                        },
                        material: Some(MaterialId::from_index(0)),
                    },
                    MeshPrimitive {
                        geometry: MeshData {
                            positions: vec![[0.0; 3]; 4],
                            indices: vec![0, 1, 2, 0, 2, 3],
                            normals: None,
                        },
                        material: None,
                    },
                ],
            }],
            materials: vec![Material {
                name: Some("Material".to_owned()),
                base_color_factor: [1.0; 4],
                base_color_texture: Some(TextureId::from_index(0)),
                double_sided: false,
            }],
            textures: vec![Texture {
                name: Some("Texture".to_owned()),
                width: 1,
                height: 1,
                rgba8: vec![255; 4],
            }],
        };

        assert_eq!(
            document.statistics(),
            AssetStatistics {
                scene_count: 1,
                node_count: 1,
                mesh_count: 1,
                primitive_count: 2,
                material_count: 1,
                texture_count: 1,
                stored_vertex_count: 7,
                stored_triangle_count: 3,
                default_scene_mesh_instance_count: 0,
                default_scene_triangle_count: 0,
            }
        );
    }

    #[test]
    fn counts_shared_mesh_for_each_default_scene_instance() {
        let asset_document = AssetDocument {
            scenes: vec![Scene {
                name: None,
                roots: vec![NodeId::from_index(0)],
            }],
            default_scene: Some(SceneId::from_index(0)),
            nodes: vec![
                Node {
                    name: None,
                    children: vec![NodeId::from_index(1)],
                    mesh: Some(MeshId::from_index(0)),
                    transform: NodeTransform::IDENTITY,
                },
                Node {
                    name: None,
                    children: Vec::new(),
                    mesh: Some(MeshId::from_index(0)),
                    transform: NodeTransform::IDENTITY,
                },
            ],
            meshes: vec![Mesh {
                name: None,
                primitives: vec![MeshPrimitive {
                    geometry: MeshData {
                        positions: vec![[0.0; 3]; 3],
                        indices: vec![0, 1, 2],
                        normals: None,
                    },
                    material: None,
                }],
            }],

            materials: Vec::new(),
            textures: Vec::new(),
        };

        let statistics = asset_document.statistics();

        assert_eq!(statistics.stored_vertex_count, 3);
        assert_eq!(statistics.stored_triangle_count, 1);
        assert_eq!(statistics.default_scene_mesh_instance_count, 2);
        assert_eq!(statistics.default_scene_triangle_count, 2);
    }
}

//! Platform-independent asset document types.
//!
//!

use std::collections::HashSet;

use crate::statistics::AssetStatistics;

/// A platform-independent representation of an inspected asset.
#[derive(Clone, Debug, PartialEq)]
pub struct AssetDocument {
    /// Scenes in source-defined order.
    pub scenes: Vec<Scene>,

    /// Scene selected as the default by the source asset, if any.
    ///
    /// The identifier indexes the `scenes` collection.
    pub default_scene: Option<SceneId>,

    /// Nodes in source-defined order.
    pub nodes: Vec<Node>,

    /// Meshes in source-defined order.
    pub meshes: Vec<Mesh>,

    /// Materials in source-defined order.
    pub materials: Vec<Material>,

    /// Textures in source-defined order.
    pub textures: Vec<Texture>,
}

/// Traverses the default scene graph using DFS and counts mesh instances and total triangles.
///
/// This function walks through all nodes in the document's default scene, starting from the
/// scene root nodes and recursively visiting children. For each node that references a mesh,
/// it increments the mesh instance count and accumulates the total triangle count across
/// all mesh primitives.
///
/// Triangles are computed by dividing the number of indices in each primitive's geometry
/// by 3 (assuming triangle-based indexing).
///
/// # Arguments
///
/// * `document` - A reference to the [`AssetDocument`] containing scenes, nodes, and meshes.
///
/// # Returns
///
/// A tuple of `(mesh_instance_count, triangle_count)`:
/// * `mesh_instance_count` - The number of nodes in the scene that reference a mesh.
/// * `triangle_count` - The total number of triangles across all mesh instances.
///
/// If the document has no default scene, returns `(0, 0)`.
fn default_scene_counts(document: &AssetDocument) -> (usize, usize) {
    let Some(scene) = document
        .default_scene
        .and_then(|scene_id| document.scene(scene_id))
    else {
        return (0, 0);
    };

    let mut pending_nodes = scene.roots.clone();
    let mut visited_nodes = HashSet::new();

    let mut mesh_instance_count = 0;
    let mut triangle_count = 0;

    while let Some(node_id) = pending_nodes.pop() {
        if !visited_nodes.insert(node_id) {
            continue;
        }

        let Some(node) = document.node(node_id) else {
            continue;
        };

        pending_nodes.extend(node.children.iter().copied());

        let Some(mesh) = node.mesh.and_then(|mesh_id| document.mesh(mesh_id)) else {
            continue;
        };

        mesh_instance_count += 1;

        triangle_count += mesh
            .primitives
            .iter()
            .map(|primitive| primitive.geometry.indices.len() / 3)
            .sum::<usize>();
    }

    (mesh_instance_count, triangle_count)
}

impl AssetDocument {
    /// Computes statistics for this asset document.
    #[must_use]
    pub fn statistics(&self) -> AssetStatistics {
        let primitive_count = self.meshes.iter().map(|mesh| mesh.primitives.len()).sum();

        let stored_vertex_count = self
            .meshes
            .iter()
            .flat_map(|mesh| mesh.primitives.iter())
            .map(|primitive| primitive.geometry.positions.len())
            .sum();

        let stored_triangle_count: usize = self
            .meshes
            .iter()
            .flat_map(|mesh| mesh.primitives.iter())
            .map(|primitive| primitive.geometry.indices.len() / 3)
            .sum();

        let (default_scene_mesh_instance_count, default_scene_triangle_count) =
            default_scene_counts(self);

        // 在这里构造并返回 AssetStatistics
        AssetStatistics {
            scene_count: self.scenes.len(),
            node_count: self.nodes.len(),
            mesh_count: self.meshes.len(),
            primitive_count,
            material_count: self.materials.len(),
            texture_count: self.textures.len(),
            stored_vertex_count,
            stored_triangle_count,
            default_scene_mesh_instance_count,
            default_scene_triangle_count,
        }
    }

    /// Returns the scene identified by `id`.
    ///
    /// Returns `None` when the index is out of bounds.
    #[must_use]
    pub fn scene(&self, id: SceneId) -> Option<&Scene> {
        self.scenes.get(id.index())
    }

    /// Returns the node identified by `id`.
    ///
    /// Returns `None` when the index is out of bounds.
    #[must_use]
    pub fn node(&self, id: NodeId) -> Option<&Node> {
        self.nodes.get(id.index())
    }

    /// Returns the mesh identified by `id`.
    ///
    /// Returns `None` when the index is out of bounds.
    #[must_use]
    pub fn mesh(&self, id: MeshId) -> Option<&Mesh> {
        self.meshes.get(id.index())
    }

    /// Returns the material identified by `id`.
    ///
    /// Returns `None` when the index is out of bounds.
    #[must_use]
    pub fn material(&self, id: MaterialId) -> Option<&Material> {
        self.materials.get(id.index())
    }

    /// Returns the texture identified by `id`.
    ///
    /// Returns `None` when the index is out of bounds.
    #[must_use]
    pub fn texture(&self, id: TextureId) -> Option<&Texture> {
        self.textures.get(id.index())
    }
}

/// Identifies a scene within an asset document.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SceneId(usize);

impl SceneId {
    /// Creates a scene identifier from a document-local index.
    #[must_use]
    pub const fn from_index(index: usize) -> Self {
        Self(index)
    }

    /// Returns the document-local scene index.
    #[must_use]
    pub const fn index(self) -> usize {
        self.0
    }
}

/// Identifies a node within an asset document.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NodeId(usize);

impl NodeId {
    /// Creates a node identifier from a document-local index.
    #[must_use]
    pub const fn from_index(index: usize) -> Self {
        Self(index)
    }

    /// Returns the document-local node index.
    #[must_use]
    pub const fn index(self) -> usize {
        self.0
    }
}

/// Identifies a mesh within an asset document.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MeshId(usize);

impl MeshId {
    /// Creates a mesh identifier from a document-local index.
    #[must_use]
    pub const fn from_index(index: usize) -> Self {
        Self(index)
    }

    /// Returns the document-local mesh index.
    #[must_use]
    pub const fn index(self) -> usize {
        self.0
    }
}

/// Identifies a material within an asset document.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MaterialId(usize);

impl MaterialId {
    /// Creates a material identifier from a document-local index.
    #[must_use]
    pub const fn from_index(index: usize) -> Self {
        Self(index)
    }

    /// Returns the document-local material index.
    #[must_use]
    pub const fn index(self) -> usize {
        self.0
    }
}

/// Identifies a texture within an asset document.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TextureId(usize);

impl TextureId {
    /// Creates a texture identifier from a document-local index.
    #[must_use]
    pub const fn from_index(index: usize) -> Self {
        Self(index)
    }

    /// Returns the document-local texture index.
    #[must_use]
    pub const fn index(self) -> usize {
        self.0
    }
}

/// A node-local translation, rotation, and scale transform.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NodeTransform {
    /// Translation along the x, y, and z axes.
    pub translation: [f32; 3],

    /// Rotation quaternion in x, y, z, w order.
    pub rotation: [f32; 4],

    /// Scale along the x, y, and z axes.
    pub scale: [f32; 3],
}

impl NodeTransform {
    /// A transform that leaves positions unchanged.
    pub const IDENTITY: Self = Self {
        translation: [0.0, 0.0, 0.0],
        rotation: [0.0, 0.0, 0.0, 1.0],
        scale: [1.0, 1.0, 1.0],
    };
}

impl Default for NodeTransform {
    fn default() -> Self {
        Self::IDENTITY
    }
}

/// A scene containing a set of root nodes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Scene {
    /// Optional human-readable scene name.
    pub name: Option<String>,

    /// Top-level nodes belonging to this scene.
    pub roots: Vec<NodeId>,
}

/// A node in the asset hierarchy.
#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    /// Optional human-readable node name.
    pub name: Option<String>,

    /// Child nodes in source-defined order.
    pub children: Vec<NodeId>,

    /// Mesh instantiated by this node, if any.
    pub mesh: Option<MeshId>,

    /// Transform relative to the parent node.
    pub transform: NodeTransform,
}

/// An indexed triangle-list primitive with an optional material assignment.
#[derive(Clone, Debug, PartialEq)]
pub struct MeshPrimitive {
    /// CPU-side vertex attributes and triangle indices.
    pub geometry: crate::MeshData,

    /// Material assigned to this primitive.
    ///
    /// Default material parameters apply when no material is assigned.
    pub material: Option<MaterialId>,
}

/// A logical mesh containing one or more renderable primitives.
#[derive(Clone, Debug, PartialEq)]
pub struct Mesh {
    /// Optional human-readable mesh name.
    pub name: Option<String>,

    /// Renderable primitives in source-defined order.
    pub primitives: Vec<MeshPrimitive>,
}

/// A material used by one or more mesh primitives.
#[derive(Clone, Debug, PartialEq)]
pub struct Material {
    /// Optional human-readable material name.
    pub name: Option<String>,

    /// Linear RGBA factor multiplied with the Base Color texture.
    pub base_color_factor: [f32; 4],

    /// Base Color texture used by this material, if any.
    pub base_color_texture: Option<TextureId>,

    /// Whether both sides of each triangle should be rendered.
    pub double_sided: bool,
}

/// A decoded two-dimensional texture resource.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Texture {
    /// Optional human-readable texture name.
    pub name: Option<String>,

    /// Texture width in pixels.
    pub width: u32,

    /// Texture height in pixels.
    pub height: u32,

    /// Decoded texels in row-major RGBA8 format.
    ///
    /// The buffer contains four bytes per pixel in red, green, blue,
    /// and alpha order.
    pub rgba8: Vec<u8>,
}

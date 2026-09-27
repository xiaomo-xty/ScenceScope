#![cfg_attr(not(debug_assertions), deny(warnings))]

//! `glTF` adapter for `SceneScope`.
//!
//!

use glam::Mat4;
use gltf::{Gltf, buffer::Source, mesh::Mode};
use libjpeg_turbo_rs::{PixelFormat, decompress_to, load_png_from_bytes};
use scenescope_core::{
    MeshData, MeshInstance,
    asset::{
        AssetDocument, Material, MaterialId, Mesh, MeshId, MeshPrimitive, Node, NodeId,
        NodeTransform, Scene, SceneId, Texture, TextureId,
    },
};
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

    #[error("mesh primitive is not a triangle list")]
    UnsupportedPrimitiveMode,

    #[error("mesh primitive has no readable POSITION attribute")]
    MissingPositions,

    #[error("mesh primitive has no readable indices")]
    MissingIndices,

    #[error("binary glTF does not contain a scene")]
    MissingScene,

    #[error("scene does not contain a mesh node")]
    MissingMeshNode,

    #[error("texture images must be embedded in the GLB BIN chunk")]
    ExternalTextureSource,

    #[error("texture buffer view is out of bounds of the binary glTF buffer")]
    TextureBufferOutBounds,

    #[error("unsupported texture image format: {0:?}")]
    UnsupportedImageFormat(String),

    #[error("failed to decode texture iamge: {0}")]
    ImageDecodeFailed(libjpeg_turbo_rs::JpegError),

    #[error("attribute {attribute} count does not match POSITION count")]
    AttributeCountMismatch { attribute: &'static str },
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

fn convert_scene(scene: &gltf::Scene<'_>) -> Scene {
    Scene {
        name: scene.name().map(str::to_owned),
        roots: scene
            .nodes()
            .map(|node| NodeId::from_index(node.index()))
            .collect(),
    }
}

fn convert_node(node: &gltf::Node<'_>) -> Node {
    let (translation, rotation, scale) = node.transform().decomposed();

    Node {
        name: node.name().map(str::to_owned),
        children: node
            .children()
            .map(|child| NodeId::from_index(child.index()))
            .collect(),
        mesh: node.mesh().map(|mesh| MeshId::from_index(mesh.index())),
        transform: NodeTransform {
            translation,
            rotation,
            scale,
        },
    }
}

fn convert_mesh(mesh: &gltf::Mesh<'_>, blob: &[u8]) -> Result<Mesh, ParseError> {
    let primitives = mesh
        .primitives()
        .map(|primitive| convert_primitive(&primitive, blob))
        .collect::<Result<Vec<_>, ParseError>>()?;

    Ok(Mesh {
        name: mesh.name().map(str::to_owned),
        primitives,
    })
}

fn convert_primitive(
    primitive: &gltf::Primitive<'_>,
    blob: &[u8],
) -> Result<MeshPrimitive, ParseError> {
    // v0.1 accepts triangle-list primitives only.
    if primitive.mode() != Mode::Triangles {
        return Err(ParseErrorKind::UnsupportedPrimitiveMode.into());
    }

    let reader = primitive.reader(|buffer| match buffer.source() {
        Source::Bin => Some(blob),
        Source::Uri(_) => None,
    });

    let positions: Vec<[f32; 3]> = reader
        .read_positions()
        .ok_or(ParseErrorKind::MissingPositions)?
        .collect();

    let indices = reader
        .read_indices()
        .ok_or(ParseErrorKind::MissingIndices)?
        .into_u32()
        .collect();

    // let normals = reader.read_normals().map(Iterator::collect);

    let normals: Option<Vec<[f32; 3]>> = reader.read_normals().map(Iterator::collect); // ← 闭包内 rustc 能推断出 collect -> Vec<[f32; 3]>

    if let Some(normals) = normals.as_deref() {
        if normals.len() != positions.len() {
            return Err(ParseErrorKind::AttributeCountMismatch {
                attribute: "NORMAL",
            }
            .into());
        }
    }

    let uvs = reader
        .read_tex_coords(0)
        .map(|coords| coords.into_f32().collect::<Vec<[f32; 2]>>());

    if let Some(uvs) = uvs.as_deref() {
        if uvs.len() != positions.len() {
            return Err(ParseErrorKind::AttributeCountMismatch {
                attribute: "TEXCOORD_0",
            }
            .into());
        }
    }

    Ok(MeshPrimitive {
        geometry: MeshData {
            positions,
            indices,
            normals,
            uvs,
        },
        material: primitive.material().index().map(MaterialId::from_index),
    })
}

fn convert_material(material: &gltf::Material<'_>) -> Material {
    let pbr = material.pbr_metallic_roughness();

    Material {
        name: material.name().map(str::to_owned),
        base_color_factor: pbr.base_color_factor(),
        base_color_texture: pbr
            .base_color_texture()
            .map(|info| TextureId::from_index(info.texture().index())),
        double_sided: material.double_sided(),
    }
}

fn convert_texture(texture: &gltf::Texture<'_>, blob: &[u8]) -> Result<Texture, ParseError> {
    let (bytes, mime_type) = match texture.source().source() {
        gltf::image::Source::View { view, mime_type } => {
            let start = view.offset();
            let end = start
                .checked_add(view.length())
                .ok_or(ParseErrorKind::TextureBufferOutBounds)?;

            let bytes = blob
                .get(start..end)
                .ok_or(ParseErrorKind::TextureBufferOutBounds)?;
            (bytes, mime_type)
        }
        gltf::image::Source::Uri { .. } => {
            return Err(ParseErrorKind::ExternalTextureSource.into());
        }
    };

    let (width, height, rgba8) = match mime_type {
        "image/png" => decode_png(bytes)?,
        "image/jpeg" => decode_jpeg(bytes)?,
        other => {
            return Err(ParseErrorKind::UnsupportedImageFormat(other.to_owned()).into());
        }
    };

    Ok(Texture {
        name: texture.name().map(str::to_owned),
        width,
        height,
        rgba8,
    })
}

fn decode_png(bytes: &[u8]) -> Result<(u32, u32, Vec<u8>), ParseError> {
    let loaded = load_png_from_bytes(bytes).map_err(ParseErrorKind::ImageDecodeFailed)?;
    let rgba8 = expand_to_rgba8(loaded.pixels, loaded.pixel_format)?;
    Ok((to_u32(loaded.width)?, to_u32(loaded.height)?, rgba8))
}

fn decode_jpeg(bytes: &[u8]) -> Result<(u32, u32, Vec<u8>), ParseError> {
    let image =
        decompress_to(bytes, PixelFormat::Rgba).map_err(ParseErrorKind::ImageDecodeFailed)?;
    Ok((to_u32(image.width)?, to_u32(image.height)?, image.data))
}

/// 将解码结果统一为 RGBA8；Grayscale/Rgb/Rgba 之外的格式拒绝。
fn expand_to_rgba8(pixels: Vec<u8>, format: PixelFormat) -> Result<Vec<u8>, ParseError> {
    match format {
        PixelFormat::Rgba => Ok(pixels),
        PixelFormat::Rgb => Ok(pixels
            .chunks_exact(3)
            .flat_map(|rgb| rgb.iter().copied().chain(std::iter::once(255)))
            .collect()),
        PixelFormat::Grayscale => Ok(pixels.into_iter().flat_map(|g| [g, g, g, 255]).collect()),
        other => Err(ParseErrorKind::UnsupportedImageFormat(format!("{other:?}")).into()),
    }
}

fn to_u32(size: usize) -> Result<u32, ParseError> {
    u32::try_from(size).map_err(|_| ParseErrorKind::TextureBufferOutBounds.into())
}

/// Parses a binary glTF asset into the internal asset document.
///
/// # Errors
///
/// Returns an error when the input is not a valid supported `GLB` file.
pub fn parse_asset_document(bytes: &[u8]) -> Result<AssetDocument, ParseError> {
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

    let scenes = gltf.scenes().map(|scene| convert_scene(&scene)).collect();

    let default_scene = gltf
        .default_scene()
        .map(|scene| SceneId::from_index(scene.index()));

    let nodes = gltf.nodes().map(|node| convert_node(&node)).collect();

    let blob = gltf
        .blob
        .as_deref()
        .ok_or(ParseErrorKind::MissingBinaryBlob)?;

    let meshes = gltf
        .meshes()
        .map(|mesh| convert_mesh(&mesh, blob))
        .collect::<Result<Vec<_>, ParseError>>()?;

    let materials: Vec<Material> = gltf
        .materials()
        .map(|material| convert_material(&material))
        .collect();

    let textures = gltf
        .textures()
        .map(|texture| convert_texture(&texture, blob))
        .collect::<Result<Vec<_>, ParseError>>()?;

    Ok(AssetDocument {
        scenes,
        default_scene,
        nodes,
        meshes,
        materials,
        textures,
    })
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

    let uvs = reader
        .read_tex_coords(0)
        .map(|coords| coords.into_f32().collect::<Vec<[f32; 2]>>());

    let mesh = MeshData {
        positions,
        indices,
        normals,
        uvs,
    };

    Ok(MeshInstance {
        mesh,
        world_transform: world_transform.to_cols_array_2d(),
    })
}

#[cfg(test)]
mod tests {

    #![allow(
        clippy::unwrap_used,
        clippy::indexing_slicing,
        reason = "test-only fixture builders and assertions on deterministic data"
    )]

    // use gltf::Mesh;
    // use glam::Mat4;
    use scenescope_core::asset::{MaterialId, MeshId, TextureId};
    use serde_json::{Map, Value, json};

    use crate::parse_asset_document;

    use super::{ParseError, ParseErrorKind, parse_first_mesh_primitive};

    /// glTF accessor componentType；数值即 WebGL 枚举（0x1406 等），见规范 §accessors。
    const FLOAT: u32 = 5126;
    const UNSIGNED_INT: u32 = 5125;

    const BOX_GLB: &[u8] = include_bytes!("../../../assets/test/Box.glb");

    const DEFAULT_UVS: [[f32; 2]; 3] = [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]];

    /// Test input parameters: control whitch features the generated GLB contains.
    /// This is a Test Data Builder pattern, but without the builder methods, since the test fixture is only used in this module.
    #[derive(Default)]
    struct FixtureOptions {
        normals: bool,
        uvs: bool,
        /// Custom UV values; may hold fewer entries than vertices to force a mismatch.
        uv_values: Vec<[f32; 2]>,
        /// Embedded BIN image: (encoded bytes, mime type)
        image: Option<(Vec<u8>, &'static str)>,
        /// Use a URI image source instead of a bufferView.
        image_uri: Option<&'static str>,
    }

    fn default_options() -> FixtureOptions {
        FixtureOptions {
            normals: true,
            uvs: true,
            ..Default::default()
        }
    }

    fn build_glb(options: &FixtureOptions) -> Vec<u8> {
        let positions: [[f32; 3]; 3] = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
        let normals: [[f32; 3]; 3] = [[0.0, 0.0, 1.0]; 3];
        let indices: [u32; 3] = [0, 1, 2];

        let mut bin: Vec<u8> = Vec::new();
        let mut buffer_views: Vec<Value> = Vec::new();
        let mut accessors: Vec<Value> = Vec::new();

        push_view(&mut bin, &mut buffer_views, &flatten_f32_3(&positions));
        accessors.push(json!({
            "bufferView": 0,
            "componentType": FLOAT,
            "count": positions.len(),
            "type": "VEC3",
            "min": [0.0, 0.0, 0.0],
            "max": [1.0, 1.0, 0.0],
        }));

        let mut attributes = Map::new();
        attributes.insert("POSITION".to_owned(), json!(0));

        // ---- NORMAL ----
        if options.normals {
            let view = push_view(&mut bin, &mut buffer_views, &flatten_f32_3(&normals));
            accessors.push(json!({
                "bufferView": view,
                "componentType": FLOAT,
                "count": normals.len(),
                "type": "VEC3",
            }));
            attributes.insert("NORMAL".to_owned(), json!(accessors.len() - 1));
        }

        // ---- TEXCOORD_0
        if options.uvs {
            let uvs: &[[f32; 2]] = if options.uv_values.is_empty() {
                &DEFAULT_UVS
            } else {
                &options.uv_values
            };

            let view = push_view(&mut bin, &mut buffer_views, &flatten_f32_2(uvs));
            accessors.push(json!({
                "bufferView": view,
                "componentType": FLOAT,
                "count": uvs.len(),
                "type": "VEC2",
            }));
            attributes.insert("TEXCOORD_0".to_owned(), json!(accessors.len() - 1));
        }

        let view = push_view(&mut bin, &mut buffer_views, &flatten_u32(&indices));
        accessors.push(json!({
            "bufferView": view,
            "componentType": UNSIGNED_INT,
            "count": indices.len(),
            "type": "SCALAR",
        }));
        let indices_accessor = accessors.len() - 1;

        // ---- 图像：两条互斥的来源分支 ----
        let mut images: Vec<Value> = Vec::new();
        let mut textures: Vec<Value> = Vec::new();
        let mut material = json!({
            "pbrMetallicRoughness": {
                "baseColorFactor": [1.0, 1.0, 1.0, 1.0],
            }
        });

        if let Some((encoded, mime_type)) = &options.image {
            // 分支一： 图像字节内嵌在BIN 里 -> 走 convert_terxture 的
            // gltf::image::Source::View 路径， PNG/JPEG 都会真正解码

            let view = push_view(&mut bin, &mut buffer_views, encoded);
            images.push(json!({ "bufferView":view, "mimeType": mime_type }));
            textures.push(json!({ "source": 0 }));
            material["pbrMetallicRoughness"]["baseColorTexture"] = json!({ "index": 0 });
        } else if let Some(uri) = options.image_uri {
            // 分支二：URI 引用外部文件 -> 应被拒绝（ExternalTextureSource）。
            // mimeType 随便写，代码在读字节之前就该报错
            images.push(json!({ "uri": uri, "mimeType": "image/png" }));
            textures.push(json!({ "source": 0 }));
            material["pbrMetallicRoughness"]["baseColorTexture"] = json!({ "index": 0 });
        }

        // ---- 根 JSON 组装：能被 Gltf::from_slice 接受的最小文档 ----
        let mut root = Map::new();
        root.insert("asset".to_owned(), json!({ "version": "2.0" }));
        root.insert("scene".to_owned(), json!(0));
        root.insert("scenes".to_owned(), json!([{ "nodes": [0] }]));
        root.insert(
            "nodes".to_owned(),
            json!([{ "mesh": 0, "name": "Triangle" }]),
        );
        root.insert(
            "meshes".to_owned(),
            json!([{
                "primitives": [{
                    "attributes": attributes,
                    "indices": indices_accessor,
                    "material": 0,
                }],
            }]),
        );
        root.insert("accessors".to_owned(), Value::Array(accessors));
        root.insert("bufferViews".to_owned(), Value::Array(buffer_views));
        // BIN chunk 无 uri 的 buffer 就是它；byteLength 与 bin 字节数一致
        root.insert("buffers".to_owned(), json!([{ "byteLength": bin.len() }]));
        root.insert("materials".to_owned(), Value::Array(vec![material]));
        if !images.is_empty() {
            root.insert("images".to_owned(), Value::Array(images));
            root.insert("textures".to_owned(), Value::Array(textures));
        }

        pack_glb(&Value::Object(root), &bin)
    }

    fn push_view(bin: &mut Vec<u8>, views: &mut Vec<Value>, bytes: &[u8]) -> usize {
        let offset = bin.len();
        bin.extend_from_slice(bytes);

        // GLB 的 BIN chunk 必须 4 字节对齐；如果不是，则填充 0。
        while !bin.len().is_multiple_of(4) {
            bin.push(0);
        }

        views.push(json!({
            "buffer": 0,
            "byteOffset": offset,
            "byteLength": bytes.len()
        }));
        views.len() - 1
    }

    fn flatten_f32_3(values: &[[f32; 3]]) -> Vec<u8> {
        values
            .iter()
            .flat_map(|v| v.iter().copied().flat_map(f32::to_le_bytes))
            .collect()
    }

    fn flatten_f32_2(values: &[[f32; 2]]) -> Vec<u8> {
        values
            .iter()
            .flat_map(|v| v.iter().copied().flat_map(f32::to_le_bytes))
            .collect()
    }

    fn flatten_u32(values: &[u32]) -> Vec<u8> {
        values.iter().flat_map(|v| v.to_le_bytes()).collect()
    }

    fn encode_png(width: u32, height: u32, color: png::ColorType, pixels: &[u8]) -> Vec<u8> {
        let mut bytes = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut bytes, width, height);
            encoder.set_color(color);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().unwrap();
            writer.write_image_data(pixels).unwrap();
        }
        bytes
    }

    fn encode_jpeg(width: usize, height: usize, rgb: &[u8]) -> Vec<u8> {
        libjpeg_turbo_rs::compress(
            rgb,
            width,
            height,
            libjpeg_turbo_rs::PixelFormat::Rgb,
            95,
            libjpeg_turbo_rs::Subsampling::S444,
        )
        .unwrap()
    }

    /// 把 glTF JSON 和 BIN 数据打包成合法的 GLB 二进制。
    ///
    /// GLB 布局（所有 u32 均为小端）：
    ///   [ 0..12 ] 文件头：magic "glTF" + version=2 + 文件总字节数
    ///   [12..20 ] JSON chunk 头：chunk 长度 + 类型 0x4E4F534A（磁盘上是 "JSON"）
    ///   [20..   ] JSON 字节，用空格 0x20 补齐到 4 的倍数
    ///   [  ..   ] BIN chunk 头：chunk 长度 + 类型 0x004E4942（磁盘上是 "BIN\0"）
    ///   [  ..   ] BIN 字节，用 0x00 补齐到 4 的倍数
    ///
    /// 两个 chunk 头里的“长度”都指补齐后的长度（含填充），这样总长度才能对账。
    fn pack_glb(root: &Value, bin: &[u8]) -> Vec<u8> {
        let mut json_bytes = serde_json::to_vec(root).unwrap();
        while !json_bytes.len().is_multiple_of(4) {
            json_bytes.push(b' ');
        }

        let mut bin_chunk = bin.to_vec();
        while !bin_chunk.len().is_multiple_of(4) {
            bin_chunk.push(0);
        }

        let json_len = u32::try_from(json_bytes.len()).unwrap();
        let bin_len = u32::try_from(bin_chunk.len()).unwrap();
        let total = 12 + 8 + json_len + 8 + bin_len;

        let mut glb = Vec::with_capacity(total as usize);

        glb.extend_from_slice(b"glTF");
        glb.extend_from_slice(&2u32.to_le_bytes());
        glb.extend_from_slice(&total.to_le_bytes());

        glb.extend_from_slice(&json_len.to_le_bytes());
        // 0x4E4F534A : "JSON"
        glb.extend_from_slice(&0x4E4F_534A_u32.to_le_bytes());
        glb.extend_from_slice(&json_bytes);

        glb.extend_from_slice(&bin_len.to_le_bytes());
        // 0x004E4942 : "BIN\0"
        glb.extend_from_slice(&0x004E_4942_u32.to_le_bytes());
        glb.extend_from_slice(&bin_chunk);

        glb
    }

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

    #[test]
    fn parse_box_asset_document() -> Result<(), ParseError> {
        let document = parse_asset_document(BOX_GLB)?;

        assert_eq!(document.scenes.len(), 1);
        assert_eq!(document.nodes.len(), 2);
        assert_eq!(document.meshes.len(), 1);

        assert_eq!(
            document
                .mesh(MeshId::from_index(0))
                .map(|mesh| mesh.primitives.len()),
            Some(1),
            "Box.glb mesh should contain one primitive"
        );

        let geometry_shape = document
            .mesh(MeshId::from_index(0))
            .and_then(|mesh| mesh.primitives.first())
            .map(|primitive| {
                (
                    primitive.geometry.positions.len(),
                    primitive.geometry.indices.len(),
                    primitive.geometry.normals.as_ref().map(Vec::len),
                )
            });

        assert_eq!(
            geometry_shape,
            Some((24, 36, Some(24))),
            "Box.glb primitive should preserve positions, indices, and normals"
        );

        assert_eq!(
            document.materials.len(),
            1,
            "Box.glb should contain one material"
        );

        let material_id = MaterialId::from_index(0);

        let primitive_material = document
            .mesh(MeshId::from_index(0))
            .and_then(|mesh| mesh.primitives.first())
            .and_then(|primitve| primitve.material);

        assert_eq!(
            primitive_material,
            Some(material_id),
            "Box.glb primitive should reference material 0"
        );

        let material_properties = document.material(material_id).map(|material| {
            (
                material.base_color_factor,
                material.base_color_texture,
                material.double_sided,
            )
        });

        assert_eq!(
            material_properties,
            Some(([0.8, 0.0, 0.0, 1.0], None, false)),
            "Box.glb material should preserve its basic properties"
        );

        Ok(())
    }

    #[test]
    fn parses_fixture_with_default_attributes() -> Result<(), ParseError> {
        let document = parse_asset_document(&build_glb(&default_options()))?;

        let Some(primitive) = document
            .mesh(MeshId::from_index(0))
            .and_then(|mesh| mesh.primitives.first())
        else {
            unreachable!("fixture always contains one primitive");
        };

        // f32 -> LE 字节 -> f32 是无损往返，可以精确断言
        assert_eq!(
            primitive.geometry.uvs.as_deref(),
            Some(DEFAULT_UVS.as_slice()),
            "fixture should preserve default UVs"
        );

        Ok(())
    }

    #[test]
    fn parses_fixture_without_optional_attributes() -> Result<(), ParseError> {
        let document = parse_asset_document(&build_glb(&FixtureOptions::default()))?;

        // 首个 primitive 的两个可选属性各自的缺失标志
        let attribtues = document
            .mesh(MeshId::from_index(0))
            .and_then(|mesh| mesh.primitives.first())
            .map(|primitive| {
                (
                    primitive.geometry.normals.is_none(),
                    primitive.geometry.uvs.is_none(),
                )
            });

        assert_eq!(
            attribtues,
            Some((true, true)),
            "missing optional attributes should stay None, not error"
        );

        Ok(())
    }

    #[test]
    fn rejects_uv_count_mismatch() {
        let options = FixtureOptions {
            uv_values: vec![[0.5, 0.5]],
            ..default_options()
        };

        let result = parse_asset_document(&build_glb(&options));

        assert!(
            matches!(
                result,
                Err(ParseError(ParseErrorKind::AttributeCountMismatch {
                    attribute: "TEXCOORD_0"
                })),
            ),
            "UV count mismatch should be rejected as AttributeCountMismatch"
        );
    }

    #[test]
    fn parses_embedded_png_base_color_texture() -> Result<(), ParseError> {
        // 2x2 像素，四角四色，像素值可以精确断言（PNG 无损）
        let pixels: [u8; 16] = [
            255, 0, 0, 255, // 红
            0, 255, 0, 255, // 绿
            0, 0, 255, 255, // 蓝
            255, 255, 0, 255, // 黄
        ];
        let options = FixtureOptions {
            image: Some((encode_png(2, 2, png::ColorType::Rgba, &pixels), "image/png")),
            ..default_options()
        };

        let document = parse_asset_document(&build_glb(&options))?;

        let texture = document
            .texture(TextureId::from_index(0))
            .map(|texture| (texture.width, texture.height, texture.rgba8.as_slice()));

        assert_eq!(
            texture,
            Some((2, 2, pixels.as_slice())),
            "embedded RGBA PNG should decode to exact RGBA8 texels"
        );

        // 材质 -> 纹理的引用链：M2-002 的关键闭环
        let material_texture = document
            .material(MaterialId::from_index(0))
            .and_then(|material| material.base_color_texture);

        assert_eq!(
            material_texture,
            Some(TextureId::from_index(0)),
            "material should reference the decoded texture"
        );

        Ok(())
    }

    #[test]
    fn expands_rgb_and_grayscale_png_to_rgba8() -> Result<(), ParseError> {
        // 3x1 的 RGB 和灰度 PNG，覆盖 expand_to_rgba8 的 Rgb/Grayscale 两个分支
        // （Rgba 分支由上一个测试覆盖）
        let rgb: [u8; 9] = [255, 0, 0, 0, 255, 0, 0, 0, 255];
        let gray: [u8; 3] = [0, 128, 255];

        let cases: [(png::ColorType, &[u8], Vec<u8>); 2] = [
            (
                png::ColorType::Rgb,
                &rgb,
                vec![255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255],
            ),
            (
                png::ColorType::Grayscale,
                &gray,
                vec![0, 0, 0, 255, 128, 128, 128, 255, 255, 255, 255, 255],
            ),
        ];

        for (color, encoded_pixels, expected_rgba8) in cases {
            let options = FixtureOptions {
                image: Some((encode_png(3, 1, color, encoded_pixels), "image/png")),
                ..default_options()
            };
            let document = parse_asset_document(&build_glb(&options))?;

            let rgba8 = document
                .texture(TextureId::from_index(0))
                .map(|texture| texture.rgba8.as_slice());

            assert_eq!(
                rgba8,
                Some(expected_rgba8.as_slice()),
                "{color:?} PNG should expand to RGBA8"
            );
        }

        Ok(())
    }

    #[test]
    fn parses_embedded_jpeg_base_color_texture() -> Result<(), ParseError> {
        // 均匀灰色块：对任何子采样/量化都稳定。JPEG 无 alpha，
        // 解码端应输出 alpha=255 的 RGBA8
        let (width, height) = (4usize, 4usize);
        let rgb = vec![128u8; width * height * 3];

        let options = FixtureOptions {
            image: Some((encode_jpeg(width, height, &rgb), "image/jpeg")),
            ..default_options()
        };

        let document = parse_asset_document(&build_glb(&options))?;

        let texture_info = document
            .texture(TextureId::from_index(0))
            .map(|texture| (texture.width, texture.height, texture.rgba8.len()));

        assert_eq!(
            texture_info,
            Some((4, 4, 64)),
            "JPEG should decode to its declared dimensions in RGBA8"
        );

        let alpha_filled = document
            .texture(TextureId::from_index(0))
            .is_some_and(|texture| texture.rgba8.chunks_exact(4).all(|px| px[3] == 255));

        assert!(
            alpha_filled,
            "JPEG has no alpha channel; the decoder should fill 255"
        );

        Ok(())
    }

    #[test]
    fn rejects_external_texture_uri() {
        let options = FixtureOptions {
            image_uri: Some("external.png"),
            ..default_options()
        };

        let result = parse_asset_document(&build_glb(&options));

        assert!(
            matches!(
                result,
                Err(ParseError(ParseErrorKind::ExternalTextureSource))
            ),
            "URI image sources should be rejected before any byte access"
        );
    }
}

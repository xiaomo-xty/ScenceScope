# 桌面端与 Web 端输出色差排查记录

- 状态：已解决
- 日期：2026-08-15
- 影响范围：`scenescope-render` 的 Surface 输出
- 关联决策：[ADR-0002：跨平台颜色输出约定](../adr/0002-cross-platform-color-output.md)

## 现象

Windows 与 Web 使用相同的 Mesh、相机、光照参数和 WGSL Shader，但最终画面的亮度和颜色不一致。几何形状与明暗分布基本相同，因此问题更可能发生在颜色写入和呈现阶段，而不是模型数据、变换或光照公式中。

两端可能获得不同的 Surface 基础格式：

```text
Windows：Bgra8UnormSrgb
Web：    Bgra8Unorm
```

如果 Render Pipeline 直接使用平台返回的基础格式，同一个线性 Shader 输出就会经历不同的编码过程。

## 根因

SceneScope 的 Fragment Shader 输出线性颜色。sRGB Render Target 会在写入时把线性值编码成 sRGB，普通 Unorm Render Target 则直接存储 Shader 输出。

以线性灰色 `0.5` 为例：

```text
sRGB Target：  0.5 -> sRGB 编码 -> 约 0.735 -> 约 #BCBCBC
Unorm Target： 0.5 -> 直接存储  -> 0.5     -> 约 #808080
```

标准 sRGB 编码函数为：

```text
C_srgb = 12.92 * C_linear                              , C_linear <= 0.0031308
C_srgb = 1.055 * C_linear^(1 / 2.4) - 0.055            , C_linear > 0.0031308
```

浏览器或系统随后按照 sRGB 解释 Surface 中的值。如果线性值 `0.5` 未经编码便被当作 sRGB 值显示，其对应的实际线性亮度只有约 `0.214`，因此画面明显偏暗。

## 四个相关概念

### `SurfaceConfiguration::format`

Surface Texture 的基础存储格式。Web 端通常使用浏览器首选的 `Bgra8Unorm`。该值不能简单地改成任意格式，必须符合 Surface 能力和平台要求。

### `SurfaceConfiguration::view_formats`

允许从 Surface Texture 创建哪些额外兼容格式的 View。它只是允许列表，不会创建 View、修改像素或改变基础格式。当前 wgpu Surface 只允许这类 View 改变格式的 sRGB 属性，例如：

```text
Bgra8Unorm <-> Bgra8UnormSrgb
```

### Surface View Format

Render Pipeline Target 和 Render Pass Attachment 实际使用的格式。使用 `Bgra8UnormSrgb` View 作为 Render Target 时，GPU 会在写入时执行线性到 sRGB 编码。

### `SurfaceConfiguration::color_space`

告诉呈现系统 Surface 中的数值属于哪个色彩空间。`SurfaceColorSpace::Srgb` 表示 BT.709 原色、D65 白点、sRGB 传递函数和 SDR 输出。

Color Space 不负责把 Shader 输出编码成 sRGB。编码由带 `Srgb` 后缀的 Texture View Format 完成。可以简记为：

```text
View Format：决定 GPU 怎么写入像素
Color Space：决定呈现系统如何解释像素
```

## 修复

### 1. 建立 sRGB 输出配置

从 Surface 基础格式取得对应的 sRGB View Format。如果不存在对应格式，则返回明确错误：

```rust
fn configure_srgb_surface_view(
    config: &mut wgpu::SurfaceConfiguration,
) -> anyhow::Result<wgpu::TextureFormat> {
    let view_format = config.format.add_srgb_suffix();

    if !view_format.is_srgb() {
        anyhow::bail!(
            "surface format {:?} has no sRGB-compatible view format",
            config.format,
        );
    }

    config.color_space = wgpu::SurfaceColorSpace::Srgb;

    if view_format != config.format {
        config.view_formats.push(view_format);
    }

    Ok(view_format)
}
```

基础格式已经是 sRGB 时，无需加入 `view_formats`，因为纹理始终允许创建与自身格式相同的 View。

### 2. 统一 Pipeline Target

Render Pipeline 的 Fragment Target 使用前一步得到的 sRGB View Format：

```rust
targets: &[Some(surface_view_format.into())],
```

### 3. 统一每帧 Texture View

从 Surface Texture 创建 View 时显式指定相同格式：

```rust
let view = surface_texture.texture.create_view(
    &wgpu::TextureViewDescriptor {
        label: Some("SceneScope sRGB surface view"),
        format: Some(self.surface_view_format),
        ..wgpu::TextureViewDescriptor::default()
    },
);
```

Pipeline Target 与 Render Pass Attachment 的 View Format 必须一致，否则会触发 wgpu Validation Error。

## 修复后的数据流

```text
Fragment Shader 输出线性颜色
              |
              v
Bgra8UnormSrgb Surface View
              |
              | GPU 执行线性到 sRGB 编码
              v
Bgra8Unorm Surface Texture
              |
              v
SurfaceColorSpace::Srgb
              |
              v
浏览器或操作系统按 sRGB 呈现
```

Fragment Shader 保持线性输出，不增加 `pow(color, 1.0 / 2.2)`。手写 Gamma 只是 sRGB 传递函数的近似，而且在 sRGB Target 上会造成重复编码。

## 验证方法

1. 记录 Windows 与 Web 的 Surface 基础格式、sRGB View Format 和 Color Space。
2. 临时让 Fragment Shader 输出 `vec4<f32>(0.5, 0.5, 0.5, 1.0)`。
3. 对两端相同区域截图取样。正确结果应接近 `#BCBCBC`；接近 `#808080` 通常表示漏掉了 sRGB 编码。
4. 恢复正常 Shader，对相同资产、相机和光照参数进行人工对比。
5. Resize 后再次取样，确认 Surface 重新配置仍然保留 sRGB 设置。

2026-08-15，Windows 与 Web 的人工对比通过，原有色差消除。精确截图取样尚未作为自动化回归测试实现。

## 排查经验

遇到跨平台颜色差异时，按以下顺序缩小范围：

1. 确认两端使用相同的资产数据、变换、相机、Shader 和光照参数。
2. 让 Fragment Shader 输出固定灰色，排除材质与光照计算。
3. 检查 Surface 基础格式是否一个带 `Srgb` 后缀、另一个不带。
4. 检查 Pipeline Target 和实际 Texture View Format 是否一致。
5. 检查 Shader 是否已经手动执行 Gamma，避免重复编码。
6. 最后再检查浏览器色彩管理、操作系统 HDR、显示器 ICC Profile 和截图工具等外部变量。

## 后续约束

- Base Color、Emissive 等颜色纹理应按 sRGB 数据读取，由采样过程解码到线性空间。
- Normal、Metallic、Roughness、Occlusion 等数据纹理必须保持线性读取。
- 光照、插值和混合在线性空间中执行。
- HDR、Tone Mapping 或广色域输出出现真实需求后，再考虑线性中间纹理与最终 Presentation Pass。

## 参考资料

- [wgpu `TextureFormat::add_srgb_suffix`](https://docs.rs/wgpu/30.0.0/wgpu/enum.TextureFormat.html#method.add_srgb_suffix)
- [wgpu `SurfaceConfiguration`](https://docs.rs/wgpu/30.0.0/wgpu/type.SurfaceConfiguration.html)
- [wgpu `SurfaceColorSpace`](https://docs.rs/wgpu/30.0.0/wgpu/enum.SurfaceColorSpace.html)
- [WebGPU `GPUCanvasConfiguration`](https://www.w3.org/TR/webgpu/#dictdef-gpucanvasconfiguration)
- [IEC sRGB transfer function summary](https://www.color.org/chardata/rgb/srgb.xalter)

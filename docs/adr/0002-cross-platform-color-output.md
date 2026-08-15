# ADR-0002：跨平台颜色输出约定

- 状态：已接受
- 日期：2026-08-12
- 决策人：项目负责人
- 落地状态：已实现（2026-08-15）

## 背景

SceneScope 的桌面端与 Web 端使用同一套 Shader，但 Surface 的首选格式可能不同。例如，桌面端可能获得 `Bgra8UnormSrgb`，浏览器端的首选 Canvas 格式可能是 `Bgra8Unorm`。如果渲染管线直接使用平台返回的格式，线性空间中的同一个颜色值可能在一端经过 sRGB 编码、在另一端未经编码便交给显示系统，造成亮度和颜色不一致。

SceneScope 需要让 Windows 与 Web 的渲染结果具备一致的颜色语义，同时保持当前 v0.1 渲染路径简单，并为后续 Base Color 纹理和更完整的光照实现建立明确约定。

## 决策

SceneScope 采用“线性工作空间 + sRGB Surface View”作为 v0.1 的颜色输出方案：

1. 光照计算、顶点颜色和 Shader 中间值在线性空间中处理。
2. Fragment Shader 输出线性颜色，不在 Shader 中手动执行 Gamma 或 sRGB 编码。
3. Surface 的基础格式继续由 wgpu 根据平台能力选择。
4. Renderer 从基础格式取得对应的 sRGB View Format；基础格式不是 sRGB 时，将该格式登记到 `SurfaceConfiguration::view_formats`。
5. Render Pipeline 的颜色 Target 与 Surface Texture 创建的 View 使用同一个 sRGB View Format，由 GPU 在写入呈现目标时执行线性到 sRGB 的转换。
6. Surface 没有可用的 sRGB 格式或兼容 View Format 时，初始化返回明确错误，不静默退回到颜色语义不一致的路径。
7. Base Color 等颜色纹理后续按 sRGB 数据读取；法线、粗糙度、金属度等非颜色数据保持线性读取。该纹理规则在相应功能接入时实施。

这项决策只规定颜色空间与呈现边界，不要求 v0.1 引入 HDR、广色域或额外的后处理 Pass。

## 实施记录

Renderer 已按本决策完成以下处理：

1. 使用 `TextureFormat::add_srgb_suffix` 从 Surface 基础格式取得兼容的 sRGB View Format。
2. 将不同于基础格式的 sRGB 格式登记到 `SurfaceConfiguration::view_formats`。
3. 将 Surface Color Space 明确设置为 `SurfaceColorSpace::Srgb`。
4. Render Pipeline 的颜色 Target 和每帧 Surface Texture View 使用同一个 sRGB View Format。
5. Resize 后沿用同一份 Surface Configuration，因此重新配置不会丢失 Color Space 或 View Format 声明。

2026-08-15，项目负责人完成人工对比，确认 Windows 与 Web 的原有色差已消除。固定灰阶取样仍保留为后续截图回归方法。

完整问题分析见[桌面端与 Web 端输出色差排查记录](../debugging/0001-desktop-web-color-difference.md)。

## 验证方式

实现后至少完成以下验证：

1. 记录 Windows 与 Web 实际选择的 Surface 基础格式、View Format 和 Color Space。
2. 临时让 Fragment Shader 输出线性灰色 `(0.5, 0.5, 0.5)`，对两端相同画面取样。经过正确 sRGB 编码后的 8-bit 结果应接近 `#BCBCBC`；接近 `#808080` 通常表示线性值未经编码便被当作 sRGB 显示。
3. 恢复正常 Shader 后，对同一资产、相机、光照参数和窗口内容比较 Windows 与 Web 截图。
4. 验证 Surface Resize 和重新配置后仍使用同一个 sRGB View Format。

截图比较需要排除操作系统显示配置、浏览器色彩管理、HDR 和显示器 ICC Profile 等外部变量；这些因素不应通过修改 Shader 数学来补偿。

## 考虑过的选项

- 依赖各平台默认 Surface Format：拒绝，因为默认格式只代表平台偏好，不保证 Windows 与 Web 具有相同的传递函数。
- 在 Fragment Shader 中使用 `pow(color, vec3(1.0 / 2.2))`：拒绝，因为幂函数只是 sRGB 传递函数的近似，还容易在 sRGB Target 上发生重复编码，并把呈现职责散落到 Shader 中。
- 先渲染到线性中间纹理，再通过最终 Presentation Pass 输出：暂缓。这是 HDR、Tone Mapping、后处理或多种输出色彩空间下更完整的方案，但超出 v0.1 当前需求。
- 接受两端轻微色差：拒绝，因为跨平台一致显示是 M1 的核心验收目标之一，且颜色空间错误会影响后续材质检查的可信度。

## 后果

好处：Shader 保持线性数学语义；桌面端与 Web 端的输出转换位置一致；后续颜色纹理、光照和材质功能有统一基础；无需为 v0.1 增加额外 Render Pass。

代价：Renderer 必须同时跟踪 Surface 基础格式和实际渲染使用的 View Format；Surface 配置、Pipeline Target 和每帧 Texture View 必须保持一致；需要为不支持兼容 sRGB View 的平台提供明确失败路径。

## 重新评估条件

- 引入 HDR、Tone Mapping、曝光控制或后处理链。
- 支持 Display-P3、Rec.2020 等广色域输出。
- 目标平台不支持基础格式对应的 sRGB View Format。
- 截图回归或浏览器实现证明 `SurfaceColorSpace` 与 Texture View 的组合存在跨平台差异。
- 材质系统接入多类颜色与非颜色纹理，需要形成独立的纹理色彩空间策略。

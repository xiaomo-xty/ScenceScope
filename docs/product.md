# SceneScope v0.1 产品规格

> 状态：执行基线。根据实现和验证证据调整，不因临时兴趣扩大范围。

## 1. 产品定义

SceneScope 是基于 Rust 与 wgpu 的跨平台 3D 资产检查和验证工具。

> SceneScope 检查资产，但不创作资产。

目标用户包括独立游戏开发者、图形程序员、技术美术，以及需要在 CI 中预检资产的团队。用户输入一个陌生 GLB 后，应能快速确认它能否正确显示、资产结构和规模是否合理，以及是否存在明显质量或预算问题。

## 2. v0.1 必须完成的结果

用户给出一个 `.glb` 文件后，SceneScope 必须能够：

1. 正确加载并显示范围内的静态场景。
2. 浏览节点层级、网格、材质和纹理摘要。
3. 查看三角形、顶点、材质和纹理统计。
4. 执行一组确定性的静态检查规则。
5. 在界面中查看诊断，并通过 CLI 导出 JSON 报告。
6. 在 Windows、Web 和 CLI 中得到一致的核心检查结果。

## 3. 用户流程

```text
选择单个 GLB
  -> 读取文件字节
  -> 校验并构建内部场景模型
  -> 显示场景、层级和统计
  -> 按 InspectionProfile 执行规则
  -> 展示诊断或由 CLI 输出 JSON
```

修复资产由外部 DCC 或建模工具完成；SceneScope 不修改输入文件。

## 4. 功能范围

### P0：v0.1 验收必需

- 单文件 `.glb`；不读取外部 `.bin`、纹理 URI 或网络资源。
- 静态三角形网格、顶点索引和节点层级。
- 节点平移、旋转和缩放（TRS）。
- `POSITION`、`NORMAL` 和 `TEXCOORD_0`。
- 内嵌 PNG/JPEG Base Color 纹理。
- 深度测试、背面剔除和轨道相机。
- Windows 和 Web 的本地文件选择。
- 节点树、网格/材质/纹理摘要和资产统计。
- 至少 5 条静态检查规则。
- 带 schema 版本的 JSON 报告。
- Windows 桌面入口、Web 入口和 CLI 入口。

### P1：不阻塞核心验收的便利能力

- Windows 文件拖放；本地文件选择仍是 P0。
- 加载过程中的进度反馈；取消选择不产生错误诊断。
- 非关键的界面筛选和排序。

## 5. 检查规则

| 规则 ID | 条件 | 默认结果 |
|---|---|---|
| `texture.dimension_limit` | 任一纹理宽或高超过 profile 限制 | warning |
| `mesh.triangle_budget` | 资产三角形总数超过 profile 限制 | warning |
| `mesh.missing_normals` | 可渲染 primitive 缺少 `NORMAL` | warning |
| `material.count_budget` | 材质总数超过 profile 限制 | warning |
| `gltf.unsupported_extension` | 使用 SceneScope 未实现、但不妨碍安全加载的扩展 | warning |

扩展处理必须区分两种情况：

- 未支持的扩展不影响范围内数据的安全加载时，生成 `gltf.unsupported_extension` 诊断并继续检查。
- `extensionsRequired` 中的扩展是正确解码所必需且 SceneScope 不支持时，返回分类为 `unsupported_feature` 的加载错误，不生成可能误导的场景报告。

诊断至少包含规则 ID、严重程度、对象位置、说明和可操作建议。相同输入、profile 和工具版本必须产生语义相同且顺序稳定的诊断；JSON 空白和缩进不属于兼容性承诺。

## 6. 异常行为

| 情况 | 预期行为 |
|---|---|
| 文件无法读取 | 平台入口显示或输出 I/O 错误，不调用检查流程 |
| 不是合法 GLB | 返回 `invalid_container`，不崩溃 |
| GLB/glTF 主版本不支持 | 返回 `unsupported_version` |
| JSON、buffer view、accessor 或索引越界 | 返回 `malformed_document` |
| 必需特性超出 v0.1 | 返回 `unsupported_feature` |
| 文件或解码资源超过保护阈值 | 返回 `resource_limit`；阈值在测量后确定 |
| 空场景 | 成功生成零值统计和明确诊断，不崩溃 |
| GPU/Surface 失败 | 由平台/渲染层分类处理，不伪装为资产诊断 |

来自文件的长度、偏移和索引必须校验。资产内容错误不得触发无说明 panic。

## 7. 报告约束

- 报告包含 schema 版本、工具版本、输入摘要、统计、诊断和总体状态。
- 严重程度为 `info`、`warning` 或 `error`；无诊断时总体状态为 `pass`。
- 默认不包含绝对路径、时间戳或运行设备等非确定信息。
- CLI 将 JSON 写入 stdout，将日志和错误写入 stderr。
- 规则 ID 和对象位置是自动化契约；message 面向人类，不作为稳定键。
- 具体 Rust 类型、JSON 字段和退出码在 M3 结合真实消费者确定，不在 M0 提前冻结。

## 8. 非功能约束

- Core 不依赖 wgpu、winit、egui、浏览器 API 或平台路径。
- Core 不暴露第三方 glTF crate 类型，也不打印检查结果。
- Renderer 只消费内部场景模型，不参与诊断决策。
- 用户资产默认只在本地进程或浏览器内存中处理，不上传、不遥测。
- 只声明经过真实构建和运行验证的平台。
- 关键状态不能只用颜色表达，主要命令可以通过键盘访问。

## 9. v0.1 验收标准

1. 同一份 Khronos `Box.glb` 在 Windows 和 Web 中方向、比例和节点变换一致。
2. 自动化测试能从 Box 提取 24 个顶点、36 个索引和 12 个三角形，并确认存在 `POSITION` 与 `NORMAL`。
3. 回归资产覆盖纹理、缺少法线、空场景、不支持扩展和损坏输入。
4. 五条规则各有至少一个通过测试和一个失败测试。
5. GUI、Web 和 CLI 对同一资产/profile 产生相同的统计与诊断集合。
6. CLI 能输出符合当前 schema 的 JSON，解析失败与检查诊断可以区分。
7. 格式、静态检查、测试和目标构建检查通过。
8. 发布 Windows 程序、可访问的 Web Demo 和 `v0.1.0` 发布说明，明确列出限制。

## 10. v0.1 不做

- 资产编辑、保存和自动修复。
- 格式转换和 GLB 之外的模型格式。
- 动画、骨骼、Morph Target 和物理。
- Draco、KTX2 和完整 glTF 扩展支持。
- 完整 PBR、阴影和高级后处理。
- 自研 ECS、完整游戏引擎或动态插件 ABI。
- 云端账号、上传、协作和商业化系统。
- 移动端和主机平台发布。

## 11. 待实现阶段确认

- M0：确认 wgpu、winit 及最低 Rust 版本并写入 lockfile。
- M1：根据首个解析测试选择 glTF 适配库和数学库。
- M2：节点树和诊断列表产生真实需求后再决定 GUI 框架。
- M3：确定 profile 文件格式、报告 JSON 字段和 CLI 退出码。
- M4：通过 benchmark 和 profiler 数据确定大文件、加载时间和内存目标。

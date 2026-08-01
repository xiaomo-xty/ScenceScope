# SceneScope 架构设计

## 1. 目标

分离资产格式适配、领域检查、渲染和平台生命周期，使 Windows、Web 和 CLI 能复用同一内部场景模型与检查规则。v0.1 的扩展能力来自清晰边界和静态规则组合，不来自动态插件。

## 2. 长期依赖方向

```text
Desktop / Web / Mobile / CLI
              |
        Application Layer
          /           \
 glTF Adapter       Renderer
          \           /
              Core
```

- 平台入口负责路径、浏览器 File、窗口、Surface 和生命周期。
- Application Layer 负责加载与检查流程的编排。
- glTF Adapter 负责校验 GLB，并转换为内部场景模型。
- Renderer 只消费内部场景模型，不执行检查规则。
- Core 保存平台无关的领域模型、统计、profile、规则和报告类型。

## 3. 当前结构

```text
crates/scenescope-core  # 空白 Core crate，尚无领域实现
apps/desktop            # 最小占位入口，尚未接入窗口和 wgpu
```

`assets/test/Box.glb` 已固定并登记许可。当前没有 glTF Adapter、Renderer、Web 或 CLI crate，也没有 GPU 实现。

## 4. 目标目录

目录只在出现真实实现时创建：

```text
crates/scenescope-core
crates/scenescope-gltf
crates/scenescope-render
apps/desktop
apps/web
apps/cli
apps/mobile          # v0.1 之后
```

## 5. 必须遵守的边界

1. Core 不依赖 wgpu、winit、egui、浏览器 API、平台路径或第三方 glTF 类型。
2. 文件以字节进入业务流程；Core 不负责打开平台路径。
3. 检查结果使用结构化数据表达，Core 不打印日志或退出进程。
4. glTF Adapter 把第三方格式类型转换为内部模型，并保留稳定的对象位置。
5. Renderer 不读取 glTF 类型，也不参与预算或质量诊断。
6. 平台入口决定文件读取、stdout/stderr、退出码和界面呈现，不改变诊断语义。
7. 第二个真实消费者出现前，不为未来复用增加抽象。

## 6. 预期数据流

```text
平台选择文件
  -> 读取 GLB bytes
  -> glTF Adapter 校验并转换
  -> AssetDocument
       -> Renderer -> 画面
       -> RuleSet  -> Diagnostic[]
  -> InspectionReport
       -> GUI/Web 展示
       -> CLI JSON
```

无法安全转换为内部模型的资产不会进入 GPU 上传或规则执行阶段。底层错误可保留为原因链，但对外必须映射为稳定分类。

## 7. 接口方向

对上层提供的检查入口最终应接近：

```rust
pub fn inspect_glb(
    bytes: &[u8],
    profile: &InspectionProfile,
) -> Result<InspectionReport, InspectError>;
```

当至少两条规则证明共同接口确有价值时，再引入：

```rust
pub trait InspectionRule {
    fn id(&self) -> &'static str;

    fn inspect(
        &self,
        asset: &AssetDocument,
        diagnostics: &mut Vec<Diagnostic>,
    );
}
```

这些是边界方向，不是 M0 已冻结的具体 Rust 类型。字段、所有权和错误细节由对应里程碑的测试与消费者决定。

## 8. 演进触发点

- 开始 GLB 到内部模型的转换测试时，创建 `scenescope-gltf`。
- Web 成为 GPU 代码的第二个消费者时，创建 `scenescope-render`。
- desktop 与 CLI 都需要同一检查编排时，再决定 Application Layer 是否独立成 crate。
- 节点树和诊断界面形成真实需求时，再选择 GUI 框架。
- `v0.1.0` 发布后，才创建移动端入口。

## 9. 测试边界

- Core：纯数据单元测试和规则的通过/失败测试。
- glTF Adapter：固定公开资产、最小 fixture、损坏字节和边界值测试。
- Renderer：变换与布局数值测试，加 Windows/Web 运行或图像证据。
- Application Layer：同一字节/profile 的统计与诊断一致性测试。
- 平台入口：启动、文件选择、Surface 恢复和退出冒烟测试。

# SceneScope

> Cross-platform 3D asset inspection and validation.

基于 Rust 与 wgpu 的跨平台 3D 资产检查和验证工具。

**产品边界：SceneScope 检查资产，但不创作资产。**

## 当前状态

项目处于 **M0：启动**。workspace 和测试资产已经就绪，桌面入口仍是最小占位程序；当前任务是创建窗口、使用 wgpu 清屏并正常退出。唯一执行状态记录在[项目路线图](docs/roadmap.md)中。

## 快速开始

前置条件：Rust 1.85 或更高版本。

```powershell
cargo run -p scenescope-desktop
```

当前命令只验证 workspace 并输出 `Hello, world!`，尚未创建图形窗口。

日常开发检查会报告警告，但不会因普通 warning 中断：

```powershell
cargo check --workspace --all-targets
cargo clippy --workspace --all-targets --all-features
cargo test --workspace --all-targets
```

release 使用严格门禁。非 debug 构建会自动将所有 rustc warning 升级为 error；`release-check` 还会以 release 配置运行全量 Clippy，并把所有 Clippy warning 当作错误：

```powershell
cargo fmt --all --check
cargo release-check
cargo test --workspace --all-targets --release
cargo build --workspace --release
cargo doc --workspace --all-features --no-deps --release
```

新增 workspace crate 时必须同时：在该 crate 的 `Cargo.toml` 加入 `[lints] workspace = true`，并在 crate 根加入 `#![cfg_attr(not(debug_assertions), deny(warnings))]`。稳定版 Cargo 暂不支持按 profile 配置 `rustflags`，因此这两处不能省略。

## Workspace

```text
crates/scenescope-core  # 平台无关 Core，当前尚无领域实现
apps/desktop            # Windows 原生入口，当前为最小占位程序
assets/test             # 固定版本的公开回归测试资产及许可记录
docs                    # 产品规格、路线图、架构和决策记录
```

长期目录见[架构设计](docs/architecture.md)。当前不建立没有真实消费者的 crate。

## v0.1 范围

- 输入单个 `.glb`，支持静态网格、索引、节点 TRS、`POSITION`、`NORMAL`、`TEXCOORD_0` 和 Base Color 纹理。
- 在 Windows 与浏览器中提供一致的场景数据和检查结果。
- 提供场景摘要、确定性规则诊断和 CLI JSON 报告。
- 首批规则：纹理尺寸、三角形预算、缺少法线、材质预算、不支持扩展。

## v0.1 不做

资产编辑/保存、格式转换、多模型格式、动画/骨骼/Morph Target/物理、Draco/KTX2、完整 PBR/阴影/后处理、ECS/完整引擎、动态插件、云端系统和主机平台均不在本期范围。

完整范围、异常语义和验收标准见[产品规格](docs/product.md)。

## 测试资产

`assets/test/Box.glb` 来自 Khronos Group 的 glTF Sample Assets，原作者/权利方为 Cesium（2017），按 CC-BY-4.0 使用。固定提交、下载地址、哈希和完整归属记录见 [资产说明](assets/test/README.md)。

## 当前限制

- 尚未实现窗口、wgpu 清屏、GLB 解析或显示。
- 尚未建立 Web 与 CLI 入口。
- 尚无模型浏览 UI、检查规则或 JSON 报告。
- 目前只在本机 Windows 环境验证；不能据此声称其他平台已受支持。

## 文档索引

- [产品规格](docs/product.md)
- [项目路线图](docs/roadmap.md)
- [架构设计](docs/architecture.md)
- [ADR-0001：项目基线](docs/adr/0001-project-foundation.md)

# ADR-0001：SceneScope 项目基线

- 状态：已接受
- 日期：2026-08-01
- 决策人：项目负责人

## 背景

SceneScope 的长期路线包含 Core、glTF Adapter、Renderer 和多个平台入口，但 M0 的真实需求只有稳定的产品边界、平台无关 Core 边界，以及能创建窗口、清屏和退出的 Windows 入口。一次性创建全部目录会产生没有消费者的抽象，并掩盖首个跨平台纵向切片。

## 决策

1. 名称在首次公开发布前保持 SceneScope，除非商标或发布条件构成阻塞。
2. 使用 Rust 2024 workspace，M0 只创建 `scenescope-core` 和 `scenescope-desktop`。
3. 使用 wgpu 和 winit 完成桌面入口；具体版本由 M0 实现和验证决定，并通过 `Cargo.lock` 固定。
4. 固定 Khronos glTF Sample Assets 的 Box GLB 到提交 `2bac6f8c57bf471df0d2a1e8a8ec023c7801dddf`，记录 CC-BY-4.0 归属和 SHA-256。
5. glTF 适配、数学、GUI 和报告序列化依赖推迟到对应里程碑的第一个真实测试。

## 考虑过的选项

- 一次性创建长期目录：拒绝，因为空 crate 无法验证边界，只增加维护成本。
- 采用完整引擎框架：拒绝，因为 v0.1 不需要 ECS 或游戏引擎能力，也会模糊检查工具的边界。
- 不提交测试资产、运行时下载：拒绝，因为测试不可离线复现且上游变动会破坏断言。
- 自制 Box：拒绝，因为公开 Khronos 样例更利于跨实现对照，但必须履行归属。

## 后果

好处：M0 脚手架构建快速，依赖方向清晰，测试资产可追溯，后续抽象有真实消费者和测试支撑。

代价：Web 成为第二个消费者时，GPU 代码可能从 desktop 迁移一次；这是为避免过早抽象而接受的局部成本。

## 重新评估条件

- Web 成为 GPU 代码第二个消费者时，提取 `scenescope-render`。
- 首个 GLB 转换测试开始时，创建 `scenescope-gltf` 并决定适配库。
- desktop 与 CLI 同时需要检查编排时，决定 Application Layer 的 crate 边界。
- 目标平台、MSRV、许可证或 wgpu/winit 兼容性发生阻塞时，重新评估具体版本。

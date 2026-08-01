# SceneScope 测试资产

## Box.glb

- 用途：M1 的首个 GLB 解析与渲染回归资产。
- 上游：Khronos Group `glTF-Sample-Assets`。
- 模型：`Models/Box/glTF-Binary/Box.glb`。
- 固定上游提交：`2bac6f8c57bf471df0d2a1e8a8ec023c7801dddf`。
- 原作者/权利方：Cesium，2017。
- 许可证：CC-BY-4.0。
- 源地址：`https://github.com/KhronosGroup/glTF-Sample-Assets/tree/2bac6f8c57bf471df0d2a1e8a8ec023c7801dddf/Models/Box`。
- 下载地址：`https://raw.githubusercontent.com/KhronosGroup/glTF-Sample-Assets/2bac6f8c57bf471df0d2a1e8a8ec023c7801dddf/Models/Box/glTF-Binary/Box.glb`。
- 文件大小：1664 bytes。
- SHA-256：`ed52f7192b8311d700ac0ce80644e3852cd01537e4d62241b9acba023da3d54e`。

PowerShell 校验命令：

```powershell
(Get-FileHash assets/test/Box.glb -Algorithm SHA256).Hash.ToLowerInvariant()
```

结果若与记录不一致，禁止静默替换资产；必须核对上游并单独更新归属、测试和 ADR。

归属：Box model, copyright 2017 Cesium, licensed under Creative Commons Attribution 4.0 International (CC-BY-4.0).

完整上游许可声明保存在 [LICENSE.Box.md](LICENSE.Box.md)。测试与发布素材使用此模型时必须保留上述归属。

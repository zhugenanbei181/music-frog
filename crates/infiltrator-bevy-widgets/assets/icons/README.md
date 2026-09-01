# 图标资产

本目录 PNG 为 Lucide 图标集的栅格化产物（chrome 集合），由 `src/icon.rs` 在运行期
经 AssetServer 路径加载（`IconId` → `icon_path` 映射）。属于源资源，必须随仓库跟踪。

- 来源：<https://lucide.dev>（<https://github.com/lucide-icons/lucide>）
- 许可：ISC（全文见 `LICENSE`）
- 上游为 SVG；本目录存的是按 UI 尺寸栅格化后的 PNG（位图不走字形码位，见
  `src/icon.rs` 的 IconPlate 管线）。

## 升级约定

- 新增/替换图标时同步确认 `LICENSE`（ISC）仍适用；ISC 仅要求保留版权声明文本。
- 图标命名沿用 Lucide kebab-case（如 `trash-2.png`、`arrow-up.png`），
  `IconId` 枚举与文件名一一对应。

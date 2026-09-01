# 字体资产

本目录四张 OFL face 由 `src/fonts.rs` 以 `include_bytes!` 编译期嵌入，属于源资源，
必须随仓库跟踪。许可证全文随目录分发（OFL 要求）。

| 文件 | 字体 | 版本 | 来源 | 许可 |
|---|---|---|---|---|
| `Inter-SemiBold.ttf` | Inter | 4.001 | <https://github.com/rsms/inter> | SIL OFL 1.1（`Inter-OFL.txt`） |
| `Inter-Regular.ttf` | Inter | 4.001 | 同上 | 同上 |
| `Inter-Medium.ttf` | Inter | 4.001 | 同上 | 同上 |
| `JetBrainsMono-Regular.ttf` | JetBrains Mono | 2.304 | <https://github.com/JetBrains/JetBrainsMono> | SIL OFL 1.1（`JetBrainsMono-OFL.txt`） |

## 升级约定

- 替换 TTF 时必须同步更新对应 `-OFL.txt`（若上游许可证文本有变）与本表版本号。
- 嵌入二进制分发视为 OFL 意义上的分发，许可证文本不得移除。
- 保留 Reserved Font Name 声明：衍生改名需遵守 OFL 第 1 条（RFN）。

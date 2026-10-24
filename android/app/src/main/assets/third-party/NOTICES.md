# MusicFrog Infiltrator — Android 第三方声明（随 APK 分发）

本文件是 Android 发行物的第三方许可披露入口，与仓库根的
`THIRD-PARTY-NOTICES.md` 保持同一事实源；冲突时以仓库根文件为准。

## GPL-3.0-or-later 组件与源码提供

Android host 的用户态隧道栈经 `tun2proxy` 0.5.4 链接 `socks5-impl`
0.5.17（**GPL-3.0-or-later**），随本 APK 编译分发。对应许可文本全文见
同目录 [`LICENSE-GPL-3.0.txt`](LICENSE-GPL-3.0.txt)。

按 GPL-3.0 第 4 条与第 6 条，对应源码提供方式为：本产品的完整对应
源码（含构建脚本与依赖锁定文件 `Cargo.lock`）随每个公开发布的
release tag 在项目仓库一并提供，release 说明中给出指向该 tag 的链接；
`socks5-impl`、`tun2proxy` 等上游组件的对应源码可由 `Cargo.lock` 中的
精确版本从 crates.io 取得。

## 其余组件

`tun2`（WTFPL）、Slint 家族（免版税变体许可）、vendored mihomo 二进制
（GPL-3.0，随 `assets/mihomo/` 分发并附其声明）与其余宽松许可组件的
逐条披露见仓库根 `THIRD-PARTY-NOTICES.md`。

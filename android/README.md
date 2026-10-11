# Android 工程地图

> 层级：L3

`:app` 提供 Compose 与 Bevy 两种 flavor；共享 Kotlin host 和 VPN/control service 位于 `app/src/main`。Bevy launcher 位于 `app/src/bevy`，NativeActivity Gradle module 位于 [bevy-host](bevy-host/README.md)。

Rust Android owner 与 UniFFI 接口见 [infiltrator-android](../crates/infiltrator-android/README.md)；Bevy driver 见 [driver README](bevy-host/driver/README.md)。本地产品章程见 [docs/README.md](docs/README.md)。

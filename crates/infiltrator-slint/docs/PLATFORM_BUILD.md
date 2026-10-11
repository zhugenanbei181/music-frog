# Slint 版本与平台构建

> 层级：L3

## 版本策略

截至 2026-10-11，Slint 官方 release 页面将 `1.18.1` 标记为 Latest，发布日期为 2026-09-21。工作区将 `slint` 和 `slint-build` 都精确固定到 `=1.18.1`，锁文件随之记录 Rust 依赖解析结果。升级时先核对官方稳定 release，再一起升级运行库和编译器 crate，并重新构建三个目标平台。

官方版本来源：[Slint releases](https://github.com/slint-ui/slint/releases)、[Slint changelog](https://github.com/slint-ui/slint/blob/master/CHANGELOG.md)。桌面依赖启用 Winit 与软件渲染；Linux release 使用 `static-openssl`，避免依赖构建机的 `libssl` soname。Android 依赖启用 `backend-android-activity-06`，使用 NativeActivity 入口。

## 构建

在安装相应 Rust target、平台 SDK 与系统图形依赖的环境运行：

```sh
# 当前 Linux 主机
cargo build --release -p infiltrator-slint --features static-openssl --bin infiltrator-slint

# Windows x64 release target (run in a Windows MSVC toolchain environment)
rustup target add x86_64-pc-windows-msvc
cargo build --release --target x86_64-pc-windows-msvc -p infiltrator-slint --bin infiltrator-slint

# Windows ARM64 release target (run in a Windows ARM64/MSVC toolchain environment)
rustup target add aarch64-pc-windows-msvc
cargo build --release --target aarch64-pc-windows-msvc -p infiltrator-slint --bin infiltrator-slint

# Android arm64-v8a + x86_64，并打包 Slint flavor APK
cd android
./gradlew :app:assembleSlintRelease
```

`assembleSlintRelease` 会执行仓库根目录的 `scripts/android-slint-build.sh`，为 `arm64-v8a` 与 `x86_64` 构建 `infiltrator-android` 的 `slint-ui` feature，再把 JNI library 放入 Slint flavor。Android 构建需要已安装的 Android SDK/NDK、JDK、Rust Android targets 和 Cargo 依赖。

CI 和发行配置应为 Linux、Windows、Android 分别构建并校验 Slint 产品身份。编译成功只证明目标可构建；安装、NativeActivity 启动、触摸操作、权限流程、运行时行为和最终发行包需要对应平台证据。没有这些证据时不得写成已验证。

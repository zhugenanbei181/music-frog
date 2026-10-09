# `:bevy-host` — Bevy Android product host (BANDROID-003/004)

> 层级：L3 · 状态：**isolated / unverified**

This directory is the intended Gradle home for the Bevy Android product host.
It is **deliberately not included in `android/settings.gradle.kts`**: the module
needs a JDK + Gradle + NDK + Rust Android toolchain to build, and wiring it in
before that is verified would risk the existing `:app` (Compose + VPN service)
build. The Rust seam it packages is already code-complete and lint-clean; only
the Gradle/NDK packaging remains environment-blocked.

## What lives where

| Concern | Owner |
| --- | --- |
| VPN `<service>` / `<receiver>` / `foregroundServiceType` | `android/app/src/main/AndroidManifest.xml` (`:app`) |
| Shared `:vpn` service binding helper | `android/app/src/main/java/.../VpnServiceBinding.kt` (`:app`) |
| Bevy host Activity (typed `NativeActivity` host) | `android/app/src/main/java/.../BevyHostActivity.kt` (`:app`) |
| Native driver (`android_main` → `launch_bevy_android_host`) | `android/bevy-host/driver` (this module) |
| Rust composition + surface pump + attach | `crates/infiltrator-android/src/product.rs`, `src/bevy_host.rs` |

## Intended build

The native driver is a `cdylib` that:

1. exports `android_main(AndroidApp)` (the `android-activity` `native-activity`
   contract),
2. sets `bevy_android::ANDROID_APP` before `DefaultPlugins`,
3. calls `infiltrator_android::launch_bevy_android_host(config)`.

`infiltrator-android` must be built with the `bevy-host` feature so the Bevy UI
renderer and the Android product composition are linked into the driver. The
resulting `libinfiltrator_bevy_android.so` is packaged under
`arm64-v8a/`, matching the `android.app.lib_name` meta-data already declared on
`BevyHostActivity`.

## Wiring checklist (not yet done — needs device/Gradle verification)

- [ ] Build the driver for `aarch64-linux-android` with `--features bevy-host`.
- [ ] Copy `libinfiltrator_bevy_android.so` into `:app`'s `jniLibs/arm64-v8a/`
      (or make `:app` depend on this module).
- [ ] Add `include(":bevy-host")` to `settings.gradle.kts` once `assembleDebug`
      for `:app` and the merged manifest are verified on an emulator.
- [ ] Select the shipping launcher surface (Compose vs Bevy) — BANDROID-003.
- [ ] Verify on a real ARM64 device: Activity start/recreate keeps the service
      bound, native text input reaches Bevy, VPN traffic and no-loopback.

Nothing in this directory is claimed verified. See
[docs/android/BEVY_ANDROID_PRODUCT.md](../../docs/android/BEVY_ANDROID_PRODUCT.md)
§2/§3/§4 and `TODO.md` `BANDROID-001`…`BANDROID-004`.

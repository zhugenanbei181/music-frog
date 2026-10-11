# Android App

> 层级：L3

`:app` packages the Compose and Bevy product flavors. `src/main` owns shared Android services, permissions and manifest declarations; `src/compose` and `src/bevy` own flavor-specific launchers and resources. The Bevy NativeActivity driver comes from `:bevy-host`.

Product identity comes from the root workspace version and `product-identity.properties`. Keep Kotlin as the Android lifecycle/IPC/UI adapter; Rust ownership and cross-process rules are in [BEVY_ANDROID_PRODUCT.md](../docs/BEVY_ANDROID_PRODUCT.md).

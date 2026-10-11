# infiltrator-ios

> 层级：L3

iOS host adapter seam：承接 Swift/Objective-C bridge、NetworkExtension、扩展进程和 Keychain 等平台生命周期。

此 crate 与 Android host 分开；不得把 `VpnService` 语义移植为 iOS 实现。

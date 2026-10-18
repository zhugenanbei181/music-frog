# lurker-template-android

> 层级：L3

Dual-engine VPN mobile client application template built for Android with Jetpack Compose,
Kotlin Coroutines, and the Lurker neutral core pipeline (`SurfaceReader` + `CommandSink`).

## Features

- **Jetpack Compose UI**: Modern, declarative UI tracking core lifecycle state in real-time.
- **Android VpnService Integration**: Clean foreground service lifecycle with `VpnService.prepare()` authorization flow.
- **Unidirectional Intent Dispatch**: Dispatches typed [`CommandIntent`] through JNI bridge into [`CommandSink`].

## Quick Start

Open the `templates/android` directory in Android Studio, or build the debug APK via Gradle:

```bash
./gradlew :app:assembleDebug
```

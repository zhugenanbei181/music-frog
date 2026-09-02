# ProGuard / R8 Rules for Infiltrator Android App

# 1. UniFFI & JNA bindings
-keep class com.sun.jna.** { *; }
-keepclassmembers class * extends com.sun.jna.** { *; }
-keep class uniffi.** { *; }
-keep class app.musicfrog.infiltrator_android.** { *; }
-keepclassmembers class app.musicfrog.infiltrator_android.** { *; }

# 2. Android NativeActivity and JNI bridge
-keepclasseswithmembernames class * {
    native <methods>;
}

# 3. Kotlin Coroutines & Reflection
-keepnames class kotlinx.coroutines.internal.MainDispatcherFactory {}
-keepnames class kotlinx.coroutines.CoroutineExceptionHandler {}

# 4. Compose Optimization
-dontwarn androidx.compose.**

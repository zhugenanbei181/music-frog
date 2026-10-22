package com.musicfrog.infiltrator

/**
 * EFFECTS-047（#190）：宿主对自己渲染 Surface 的 typed 报告。
 *
 * 线码与 Rust 侧的 `HostSurfaceState` 一一对应。Kotlin 只观测平台的
 * `SurfaceHolder` 回调并把事实推给 Rust，Surface 生命周期本身由 Rust 侧
 * 的唯一真源裁决，因此这里不再保存第二份状态。
 */
internal enum class NativeSurfaceState(val wireCode: Int) {
    /** Surface 已创建，平台还没有给出尺寸。 */
    Created(0),

    /** Surface 可用，尺寸由平台在尺寸回调里给出。 */
    Live(1),

    /** Surface 已被平台销毁。 */
    Destroyed(2),
}

/**
 * EFFECTS-047（#192）：宿主窗口的方向。
 *
 * 线码与 Rust 侧的 `HostOrientation` 一一对应。
 */
internal enum class NativeWindowOrientation(val wireCode: Int) {
    /** 竖屏。 */
    Portrait(0),

    /** 横屏。 */
    Landscape(1),

    /** 正方形窗口（折叠屏展开等形态）。 */
    Square(2),
}

/**
 * 由真实像素尺寸判定窗口方向。
 *
 * 平台的方向常量在旧 API 上被标记为过时，因此方向由宿主自己观测到的
 * 像素尺寸推导：宽大于高是横屏，高大于宽是竖屏，相等是正方形窗口。
 */
internal fun nativeOrientationFor(widthPx: Int, heightPx: Int): NativeWindowOrientation = when {
    widthPx > heightPx -> NativeWindowOrientation.Landscape
    heightPx > widthPx -> NativeWindowOrientation.Portrait
    else -> NativeWindowOrientation.Square
}

/**
 * Rust 侧对一条 Surface 事实的 typed 回执。
 *
 * 线码与 Rust 的 `FfiErrorCode` 对齐：0 是接受，1 是「事实来自已经退役的
 * Activity 代」，其余都是失败。
 */
internal enum class NativeSurfaceReceipt {
    /** 事实被接受。 */
    Accepted,

    /** 事实来自已经退役的 Activity 代，被忽略。 */
    Stale,

    /** 调用失败。 */
    Failed,
}

/** 解码 Rust 对一条 Surface 事实的回执。 */
internal fun nativeSurfaceReceiptFrom(code: Int): NativeSurfaceReceipt = when (code) {
    0 -> NativeSurfaceReceipt.Accepted
    1 -> NativeSurfaceReceipt.Stale
    else -> NativeSurfaceReceipt.Failed
}

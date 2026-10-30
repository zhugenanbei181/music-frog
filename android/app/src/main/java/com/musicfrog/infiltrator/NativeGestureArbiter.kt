package com.musicfrog.infiltrator

/**
 * EFFECTS-047（#191）：宿主上报的原始触摸阶段。
 *
 * 线码与 Rust 侧的 `TouchPhase` 一一对应。
 */
internal enum class NativeTouchPhase(val wireCode: Int) {
    /** 指针按下。 */
    Started(0),

    /** 指针移动。 */
    Moved(1),

    /** 指针抬起。 */
    Ended(2),

    /** 手势被平台取消。 */
    Canceled(3),
}

/**
 * EFFECTS-047（#191）：手势仲裁的 typed 结果。
 *
 * 线码与 Rust 侧的 `GestureOutcome` 一一对应。仲裁规则（10px 位移容差、
 * 方向判定与超时判定）只存在于 Rust 侧，这里只是把判决读回 typed 结果。
 */
internal enum class NativeGestureOutcome(val wireCode: Int) {
    /** 还没有结论：手势仍在进行。 */
    Undecided(0),

    /** 点击：宿主可以派发子项动作。 */
    Tap(1),

    /** 长按：指针在容差内停留超过长按超时。 */
    LongPress(2),

    /** 纵向滑动。 */
    ScrollVertical(3),

    /** 横向滑动。 */
    ScrollHorizontal(4),

    /** 多点触控：第二个指针加入，这次手势不可能是点击。 */
    MultiTouch(5),

    /** 手势被平台取消。 */
    Canceled(6),
}

/**
 * 这次判决是否阻断列表子项的动作。
 *
 * 滑动、长按、多点触控与取消都阻断；「未决」不阻断，因此宿主在仲裁器
 * 不可用时仍然保留平台默认的点击行为，而不是把用户的点击吞掉。
 */
internal fun NativeGestureOutcome.blocksRowAction(): Boolean = when (this) {
    NativeGestureOutcome.Undecided, NativeGestureOutcome.Tap -> false

    NativeGestureOutcome.LongPress,
    NativeGestureOutcome.ScrollVertical,
    NativeGestureOutcome.ScrollHorizontal,
    NativeGestureOutcome.MultiTouch,
    NativeGestureOutcome.Canceled,
    -> true
}

/**
 * 把 Rust 的线码解回 typed 结果。
 *
 * 未知线码按「未决」处理：它不阻断子项动作，宿主因此不会因为一次无法
 * 识别的判决而吞掉用户的点击。
 */
internal fun nativeGestureOutcomeFrom(code: Int): NativeGestureOutcome = when (code) {
    NativeGestureOutcome.Tap.wireCode -> NativeGestureOutcome.Tap
    NativeGestureOutcome.LongPress.wireCode -> NativeGestureOutcome.LongPress
    NativeGestureOutcome.ScrollVertical.wireCode -> NativeGestureOutcome.ScrollVertical
    NativeGestureOutcome.ScrollHorizontal.wireCode -> NativeGestureOutcome.ScrollHorizontal
    NativeGestureOutcome.MultiTouch.wireCode -> NativeGestureOutcome.MultiTouch
    NativeGestureOutcome.Canceled.wireCode -> NativeGestureOutcome.Canceled
    else -> NativeGestureOutcome.Undecided
}

/**
 * 一次归一化的指针采样。
 *
 * 坐标是物理像素，与 Rust 侧的 10px 位移容差同一坐标系。
 */
internal data class NativeTouchSample(val pointerId: Long, val phase: NativeTouchPhase, val x: Float, val y: Float, val timeMillis: Long)

/**
 * EFFECTS-047（#191）：把手势采样转发给 Rust 的 typed 仲裁器。
 *
 * 仲裁器是宿主侧唯一的适配层：它只搬运采样、读回判决，判定规则完全在
 * Rust 侧，因此 Compose 表面不需要（也不允许）自己推导点击与滑动的区别。
 */
internal class NativeGestureArbiter {
    private var lastOutcome: NativeGestureOutcome = NativeGestureOutcome.Undecided

    /** 最近一次仲裁判决。 */
    val outcome: NativeGestureOutcome
        get() = lastOutcome

    /** 这次判决是否阻断列表子项的动作。 */
    val blocksRowAction: Boolean
        get() = lastOutcome.blocksRowAction()

    /**
     * 送入一个采样并读回 typed 判决。
     *
     * 原生库不可用时线码是未知的错误码，判决因此停在「未决」，宿主保留
     * 平台默认的点击行为，而不是把点击吞掉。
     */
    fun onSample(sample: NativeTouchSample): NativeGestureOutcome {
        val code = RustBridge.onGestureSample(
            sample.pointerId,
            sample.phase.wireCode,
            sample.x,
            sample.y,
            sample.timeMillis,
        )
        lastOutcome = nativeGestureOutcomeFrom(code)
        return lastOutcome
    }
}

package com.musicfrog.infiltrator.ui.common

import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.input.pointer.PointerEventPass
import androidx.compose.ui.input.pointer.pointerInput
import com.musicfrog.infiltrator.NativeGestureArbiter
import com.musicfrog.infiltrator.NativeTouchPhase
import com.musicfrog.infiltrator.NativeTouchSample

/**
 * EFFECTS-047（#191）：记住一行的手势仲裁器。
 *
 * 每一行有自己的仲裁器，因此并发的两行不会共享同一次判决。
 */
@Composable
internal fun rememberRowGestureArbiter(): NativeGestureArbiter = remember { NativeGestureArbiter() }

/**
 * EFFECTS-047（#191）：把一次指针手势喂给宿主的 typed 仲裁器。
 *
 * 这里只搬运采样与读回判决：位移容差、方向判定与超时判定都在 Rust 侧的
 * 唯一真源里，Compose 的回调里没有任何手势规则。
 *
 * 观察发生在 `Initial` 指针事件阶段，它早于平台自己的点击阶段，因此
 * 点击回调读到的判决一定是这一次手势的判决，而不是上一次留下的残值。
 */
internal fun Modifier.arbitratedRowGesture(arbiter: NativeGestureArbiter): Modifier = pointerInput(arbiter) {
    awaitPointerEventScope {
        while (true) {
            val down = awaitFirstDown(requireUnconsumed = false, pass = PointerEventPass.Initial)
            val pointerId = down.id.value
            arbiter.onSample(
                NativeTouchSample(
                    pointerId = pointerId,
                    phase = NativeTouchPhase.Started,
                    x = down.position.x,
                    y = down.position.y,
                    timeMillis = down.uptimeMillis,
                ),
            )
            var settled = false
            while (!settled) {
                val event = awaitPointerEvent(PointerEventPass.Initial)
                val change = event.changes.firstOrNull { it.id == down.id }
                if (change == null) {
                    // The platform took the pointer away: the gesture is over
                    // and it can never become a row action.
                    arbiter.onSample(
                        NativeTouchSample(
                            pointerId = pointerId,
                            phase = NativeTouchPhase.Canceled,
                            x = down.position.x,
                            y = down.position.y,
                            timeMillis = down.uptimeMillis,
                        ),
                    )
                    settled = true
                } else {
                    val phase = if (change.pressed) NativeTouchPhase.Moved else NativeTouchPhase.Ended
                    arbiter.onSample(
                        NativeTouchSample(
                            pointerId = pointerId,
                            phase = phase,
                            x = change.position.x,
                            y = change.position.y,
                            timeMillis = change.uptimeMillis,
                        ),
                    )
                    settled = !change.pressed
                }
            }
        }
    }
}

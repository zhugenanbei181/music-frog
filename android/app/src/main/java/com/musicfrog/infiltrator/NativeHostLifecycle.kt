package com.musicfrog.infiltrator

/** Android Activity lifecycle phase mirrored by the Rust host contract. */
internal enum class NativeActivityPhase(val wireCode: Int) {
    Created(0),
    Started(1),
    Resumed(2),
    Paused(3),
    Stopped(4),
    Destroyed(5),
}

internal enum class NativeActivityPhaseTransition {
    Unchanged,
    Advanced,
    Rejected,
}

internal fun NativeActivityPhase.transitionTo(
    next: NativeActivityPhase,
): NativeActivityPhaseTransition = when (this) {
    NativeActivityPhase.Created -> when (next) {
        NativeActivityPhase.Created -> NativeActivityPhaseTransition.Unchanged
        NativeActivityPhase.Started,
        NativeActivityPhase.Destroyed -> NativeActivityPhaseTransition.Advanced
        NativeActivityPhase.Resumed,
        NativeActivityPhase.Paused,
        NativeActivityPhase.Stopped -> NativeActivityPhaseTransition.Rejected
    }
    NativeActivityPhase.Started -> when (next) {
        NativeActivityPhase.Started -> NativeActivityPhaseTransition.Unchanged
        NativeActivityPhase.Resumed,
        NativeActivityPhase.Stopped,
        NativeActivityPhase.Destroyed -> NativeActivityPhaseTransition.Advanced
        NativeActivityPhase.Created,
        NativeActivityPhase.Paused -> NativeActivityPhaseTransition.Rejected
    }
    NativeActivityPhase.Resumed -> when (next) {
        NativeActivityPhase.Resumed -> NativeActivityPhaseTransition.Unchanged
        NativeActivityPhase.Paused,
        NativeActivityPhase.Stopped,
        NativeActivityPhase.Destroyed -> NativeActivityPhaseTransition.Advanced
        NativeActivityPhase.Created,
        NativeActivityPhase.Started -> NativeActivityPhaseTransition.Rejected
    }
    NativeActivityPhase.Paused -> when (next) {
        NativeActivityPhase.Paused -> NativeActivityPhaseTransition.Unchanged
        NativeActivityPhase.Resumed,
        NativeActivityPhase.Stopped,
        NativeActivityPhase.Destroyed -> NativeActivityPhaseTransition.Advanced
        NativeActivityPhase.Created,
        NativeActivityPhase.Started -> NativeActivityPhaseTransition.Rejected
    }
    NativeActivityPhase.Stopped -> when (next) {
        NativeActivityPhase.Stopped -> NativeActivityPhaseTransition.Unchanged
        NativeActivityPhase.Started,
        NativeActivityPhase.Destroyed -> NativeActivityPhaseTransition.Advanced
        NativeActivityPhase.Created,
        NativeActivityPhase.Resumed,
        NativeActivityPhase.Paused -> NativeActivityPhaseTransition.Rejected
    }
    NativeActivityPhase.Destroyed -> when (next) {
        NativeActivityPhase.Destroyed -> NativeActivityPhaseTransition.Unchanged
        NativeActivityPhase.Created,
        NativeActivityPhase.Started,
        NativeActivityPhase.Resumed,
        NativeActivityPhase.Paused,
        NativeActivityPhase.Stopped -> NativeActivityPhaseTransition.Rejected
    }
}

/** Window focus is independent from Activity lifecycle and may be unknown. */
internal enum class NativeWindowFocus(val wireCode: Int) {
    Unknown(0),
    Focused(1),
    Unfocused(2),
}

/** One coherent observation sent over JNI; visibility is derived from phase. */
internal data class NativeHostLifecycleObservation(
    val generation: Long,
    val phase: NativeActivityPhase,
    val focus: NativeWindowFocus,
)

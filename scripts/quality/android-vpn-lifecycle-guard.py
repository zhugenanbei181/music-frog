#!/usr/bin/env python3
"""Guard the typed, bounded WakeLock integration in Android VPN lifecycle tasks."""

from __future__ import annotations

import argparse
from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parents[2]
SERVICE = Path("android/app/src/main/java/com/musicfrog/infiltrator/MihomoVpnService.kt")
LEASE = Path("android/app/src/main/java/com/musicfrog/infiltrator/VpnLifecycleWakeLock.kt")
ANDROID_HANDLE = Path("android/app/src/main/java/com/musicfrog/infiltrator/AndroidVpnWakeLockHandle.kt")


def between(source: str, start_marker: str, end_marker: str) -> str | None:
    start = source.find(start_marker)
    if start < 0:
        return None
    end = source.find(end_marker, start + len(start_marker))
    if end < 0:
        return None
    return source[start:end]


def task_problems(source: str, task: str, finish_call: str) -> list[str]:
    problems: list[str] = []
    segment = (
        between(source, "private inner class StartTransactionTask", "private fun runStartTransaction")
        if task == "StartTransactionTask"
        else between(source, "private inner class CleanupTransactionTask", "private fun rejectCleanupAdmission")
    )
    if segment is None:
        return [f"MihomoVpnService.kt: {task} body is missing or not bounded by its owner methods"]
    if segment.count("lifecycleWakeLock.withLease(") != 1:
        problems.append(f"MihomoVpnService.kt: {task} must execute exactly once through lifecycleWakeLock.withLease")
    for required in ("elapsedRealtimeNow = { SystemClock.elapsedRealtime() }", "onAcquire =", "onRelease ="):
        if required not in segment:
            problems.append(f"MihomoVpnService.kt: {task} must provide `{required}` to the typed lease")
    if "finally" not in segment or finish_call not in segment:
        problems.append(f"MihomoVpnService.kt: {task} must finish service admission in a finally path")
    return problems


def source_problems_from_sources(sources: dict[str, str]) -> list[str]:
    problems: list[str] = []
    service = sources.get(SERVICE.as_posix(), "")
    lease = sources.get(LEASE.as_posix(), "")
    android_handle = sources.get(ANDROID_HANDLE.as_posix(), "")

    problems.extend(task_problems(service, "StartTransactionTask", "serviceAdmission.finishStart(generation)"))
    problems.extend(task_problems(service, "CleanupTransactionTask", "serviceAdmission.finishCleanup(request)"))

    for required in (
        "sealed interface VpnWakeLockState",
        "data object Released",
        "data class Held(",
        "fun <T> withLease(",
        "finally",
        "onRelease(release(",
        "MAX_LEASE_DURATION_MILLIS = 45_000L",
    ):
        if required not in lease:
            problems.append(f"{LEASE}: typed lease must contain `{required}`")
    for required in ("wakeLock.acquire(timeoutMillis)", "if (wakeLock.isHeld)", "wakeLock.release()"):
        if required not in android_handle:
            problems.append(f"{ANDROID_HANDLE}: Android platform adapter must contain `{required}`")
    return problems


def source_problems(root: Path = ROOT) -> list[str]:
    sources: dict[str, str] = {}
    for relative in (SERVICE, LEASE, ANDROID_HANDLE):
        path = root / relative
        try:
            sources[relative.as_posix()] = path.read_text(encoding="utf-8")
        except OSError as error:
            return [f"{path}: could not read VPN lifecycle source: {error}"]
    return source_problems_from_sources(sources)


def self_test() -> bool:
    valid = {
        SERVICE.as_posix(): """
            private inner class StartTransactionTask : Runnable {
                override fun run() {
                    try {
                        lifecycleWakeLock.withLease(
                            elapsedRealtimeNow = { SystemClock.elapsedRealtime() },
                            onAcquire = {}, onRelease = {},
                        ) { runStartTransaction() }
                    } finally { serviceAdmission.finishStart(generation) }
                }
            }
            private fun runStartTransaction() {}
            private inner class CleanupTransactionTask : Runnable {
                override fun run() {
                    try {
                        lifecycleWakeLock.withLease(
                            elapsedRealtimeNow = { SystemClock.elapsedRealtime() },
                            onAcquire = {}, onRelease = {},
                        ) { finishCleanup() }
                    } finally { serviceAdmission.finishCleanup(request) }
                }
            }
            private fun rejectCleanupAdmission() {}
        """,
        LEASE.as_posix(): """
            sealed interface VpnWakeLockState {
                data object Released
                data class Held(val generation: Long)
            }
            const val MAX_LEASE_DURATION_MILLIS = 45_000L
            fun <T> withLease() { try {} finally { onRelease(release()) } }
        """,
        ANDROID_HANDLE.as_posix(): "wakeLock.acquire(timeoutMillis); if (wakeLock.isHeld) wakeLock.release()",
    }
    if source_problems_from_sources(valid):
        return False
    stale = dict(valid)
    stale[SERVICE.as_posix()] = valid[SERVICE.as_posix()].replace("lifecycleWakeLock.withLease(", "")
    return any("StartTransactionTask must execute exactly once" in issue
               for issue in source_problems_from_sources(stale))


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args(argv)
    if args.self_test:
        if self_test():
            print("Android VPN lifecycle guard self-test: passed")
            return 0
        print("Android VPN lifecycle guard self-test: failed", file=sys.stderr)
        return 1
    problems = source_problems()
    if problems:
        for problem in problems:
            print(f"VIOLATION: {problem}", file=sys.stderr)
        return 1
    print("Android VPN lifecycle guard: passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

package com.musicfrog.infiltrator

import infiltrator_android.AndroidHostReply
import infiltrator_android.FfiErrorCode
import org.json.JSONObject

internal enum class VpnStopFailureCode {
    InvalidState,
    NotReady,
}

internal data class VpnStopFailureWire(val code: VpnStopFailureCode, val message: String, val retryable: Boolean) {
    fun toJson(): JSONObject = JSONObject()
        .put("code", code.name)
        .put("message", message)
        .put("retryable", retryable)
}

/** Sum type matching the Rust Serde-tagged VpnOwnerStopReceipt contract. */
internal sealed interface VpnOwnerStopReceiptWire {
    data class Stopped(val snapshot: VpnOwnerSnapshotWire) : VpnOwnerStopReceiptWire
    data class OwnerUnavailable(val failure: VpnStopFailureWire) : VpnOwnerStopReceiptWire
    data class OwnerUnavailableWithReadback(val failure: VpnStopFailureWire, val snapshot: VpnOwnerSnapshotWire) : VpnOwnerStopReceiptWire

    data class Failed(val failure: VpnStopFailureWire) : VpnOwnerStopReceiptWire
    data class FailedWithReadback(val failure: VpnStopFailureWire, val snapshot: VpnOwnerSnapshotWire) : VpnOwnerStopReceiptWire

    data class TimedOut(val failure: VpnStopFailureWire) : VpnOwnerStopReceiptWire
    data class TimedOutWithReadback(val failure: VpnStopFailureWire, val snapshot: VpnOwnerSnapshotWire) : VpnOwnerStopReceiptWire

    data class Superseded(val failure: VpnStopFailureWire) : VpnOwnerStopReceiptWire
    data class SupersededWithReadback(val failure: VpnStopFailureWire, val snapshot: VpnOwnerSnapshotWire) : VpnOwnerStopReceiptWire

    data class Canceled(val failure: VpnStopFailureWire) : VpnOwnerStopReceiptWire
    data class CanceledWithReadback(val failure: VpnStopFailureWire, val snapshot: VpnOwnerSnapshotWire) : VpnOwnerStopReceiptWire
}

internal fun vpnOwnerStopReceiptJson(receipt: VpnOwnerStopReceiptWire): String = when (receipt) {
    is VpnOwnerStopReceiptWire.Stopped -> receiptJson("Stopped")
        .put("snapshot", JSONObject(receipt.snapshot.canonicalJson))
        .toString()

    is VpnOwnerStopReceiptWire.OwnerUnavailable -> failedReceiptJson(
        "OwnerUnavailable",
        receipt.failure,
    ).toString()

    is VpnOwnerStopReceiptWire.OwnerUnavailableWithReadback -> failedReceiptJson(
        "OwnerUnavailableWithReadback",
        receipt.failure,
    ).put("snapshot", JSONObject(receipt.snapshot.canonicalJson)).toString()

    is VpnOwnerStopReceiptWire.Failed -> failedReceiptJson(
        "Failed",
        receipt.failure,
    ).toString()

    is VpnOwnerStopReceiptWire.FailedWithReadback -> failedReceiptJson(
        "FailedWithReadback",
        receipt.failure,
    ).put("snapshot", JSONObject(receipt.snapshot.canonicalJson)).toString()

    is VpnOwnerStopReceiptWire.TimedOut -> failedReceiptJson(
        "TimedOut",
        receipt.failure,
    ).toString()

    is VpnOwnerStopReceiptWire.TimedOutWithReadback -> failedReceiptJson(
        "TimedOutWithReadback",
        receipt.failure,
    ).put("snapshot", JSONObject(receipt.snapshot.canonicalJson)).toString()

    is VpnOwnerStopReceiptWire.Superseded -> failedReceiptJson(
        "Superseded",
        receipt.failure,
    ).toString()

    is VpnOwnerStopReceiptWire.SupersededWithReadback -> failedReceiptJson(
        "SupersededWithReadback",
        receipt.failure,
    ).put("snapshot", JSONObject(receipt.snapshot.canonicalJson)).toString()

    is VpnOwnerStopReceiptWire.Canceled -> failedReceiptJson(
        "Canceled",
        receipt.failure,
    ).toString()

    is VpnOwnerStopReceiptWire.CanceledWithReadback -> failedReceiptJson(
        "CanceledWithReadback",
        receipt.failure,
    ).put("snapshot", JSONObject(receipt.snapshot.canonicalJson)).toString()
}

private fun failedReceiptJson(terminal: String, failure: VpnStopFailureWire): JSONObject =
    receiptJson(terminal).put("failure", failure.toJson())

private fun receiptJson(terminal: String): JSONObject = JSONObject().put("receipt", terminal)

internal fun VpnStopResult.toOwnerStopReceipt(evidence: VpnStopReadbackEvidence): VpnOwnerStopReceiptWire = when (this) {
    VpnStopResult.Stopped -> when (evidence) {
        VpnStopReadbackEvidence.Missing -> failedReceipt(
            StopFailureKind.Failed,
            failure(VpnStopFailureCode.InvalidState, "VPN stop completed without its owner snapshot"),
            evidence,
        )

        is VpnStopReadbackEvidence.OwnerSnapshot -> if (evidence.snapshot.isTerminalStopped()) {
            VpnOwnerStopReceiptWire.Stopped(evidence.snapshot)
        } else {
            failedReceipt(
                StopFailureKind.Failed,
                failure(
                    VpnStopFailureCode.InvalidState,
                    "VPN stop readback did not confirm released resources",
                ),
                evidence,
            )
        }
    }

    is VpnStopResult.Failed -> failedReceipt(
        StopFailureKind.Failed,
        failure(VpnStopFailureCode.InvalidState, message),
        evidence,
    )

    VpnStopResult.TimedOut -> failedReceipt(
        StopFailureKind.TimedOut,
        failure(VpnStopFailureCode.NotReady, "VPN owner stop timed out"),
        evidence,
    )

    VpnStopResult.Superseded -> failedReceipt(
        StopFailureKind.Superseded,
        failure(VpnStopFailureCode.InvalidState, "VPN stop target was superseded"),
        evidence,
    )

    VpnStopResult.Canceled -> failedReceipt(
        StopFailureKind.Canceled,
        failure(VpnStopFailureCode.NotReady, "VPN stop request was canceled before readback"),
        evidence,
    )
}

/** Terminal Messenger reply plus its same-request owner-authored VPN snapshot. */
internal data class VpnStopOwnerReadback(
    val reply: AndroidHostReply,
    val statusCode: Int,
    val evidence: VpnStopReadbackEvidence,
    val terminal: OwnerClientTerminal,
)

internal sealed interface VpnStopReadbackEvidence {
    data object Missing : VpnStopReadbackEvidence

    data class OwnerSnapshot(val snapshot: VpnOwnerSnapshotWire) : VpnStopReadbackEvidence
}

private enum class StopFailureKind {
    OwnerUnavailable,
    Failed,
    TimedOut,
    Superseded,
    Canceled,
}

internal fun VpnStopOwnerReadback.toReceipt(): VpnOwnerStopReceiptWire {
    val evidence = this.evidence
    return when (terminal) {
        OwnerClientTerminal.Interrupted -> failedReceipt(
            StopFailureKind.Canceled,
            failure(VpnStopFailureCode.NotReady, "VPN stop request was canceled before readback"),
            evidence,
        )

        OwnerClientTerminal.ReplyTimedOut -> failedReceipt(
            StopFailureKind.TimedOut,
            failure(VpnStopFailureCode.NotReady, "VPN owner stop timed out"),
            evidence,
        )

        OwnerClientTerminal.OwnerUnavailable,
        OwnerClientTerminal.BindRejected,
        OwnerClientTerminal.NullBinding,
        OwnerClientTerminal.BinderDied,
        OwnerClientTerminal.BindingDied,
        OwnerClientTerminal.SendFailed,
        OwnerClientTerminal.PendingCapacityExceeded,
        -> failedReceipt(
            StopFailureKind.OwnerUnavailable,
            failure(VpnStopFailureCode.NotReady, "Android VPN owner is unavailable during stop"),
            evidence,
        )

        OwnerClientTerminal.OwnerSessionStale -> failedReceipt(
            StopFailureKind.Superseded,
            failure(VpnStopFailureCode.InvalidState, "VPN stop owner session was superseded"),
            evidence,
        )

        OwnerClientTerminal.OwnerSessionReadbackRequired -> when (reply) {
            AndroidHostReply.SERVICE_UNAVAILABLE -> failedReceipt(
                StopFailureKind.OwnerUnavailable,
                failure(VpnStopFailureCode.NotReady, "Android VPN owner readback is unavailable"),
                evidence,
            )

            else -> failedReceipt(
                StopFailureKind.Failed,
                failure(VpnStopFailureCode.NotReady, "VPN stop requires a current owner session readback"),
                evidence,
            )
        }

        OwnerClientTerminal.InvalidReply,
        OwnerClientTerminal.CoreSnapshotStale,
        OwnerClientTerminal.VpnSnapshotStale,
        OwnerClientTerminal.OwnerDataStale,
        -> failedReceipt(
            StopFailureKind.Failed,
            failure(VpnStopFailureCode.NotReady, "VPN stop reply failed its owner identity or revision fence"),
            evidence,
        )

        OwnerClientTerminal.ReplyReceived -> reply.toReceipt(statusCode, evidence)
    }
}

internal fun VpnStopOwnerReadback.toReceiptJson(): String = vpnOwnerStopReceiptJson(toReceipt())

private fun AndroidHostReply.toReceipt(statusCode: Int, evidence: VpnStopReadbackEvidence): VpnOwnerStopReceiptWire = when (this) {
    AndroidHostReply.VPN_STOPPED -> when (evidence) {
        VpnStopReadbackEvidence.Missing -> failedReceipt(
            StopFailureKind.Failed,
            failure(VpnStopFailureCode.NotReady, "VPN stop reply omitted its owner readback"),
            evidence,
        )

        is VpnStopReadbackEvidence.OwnerSnapshot -> when {
            statusCode != FfiErrorCode.OK.value.toInt() -> failedReceipt(
                StopFailureKind.Failed,
                failure(VpnStopFailureCode.InvalidState, "VPN stop owner returned a non-success status"),
                evidence,
            )

            evidence.snapshot.isTerminalStopped() -> VpnOwnerStopReceiptWire.Stopped(evidence.snapshot)

            else -> failedReceipt(
                StopFailureKind.Failed,
                failure(
                    VpnStopFailureCode.InvalidState,
                    "VPN stop readback did not confirm released resources",
                ),
                evidence,
            )
        }
    }

    AndroidHostReply.VPN_STOP_TIMED_OUT -> if (statusCode == FfiErrorCode.NOT_READY.value.toInt()) {
        terminalReceiptRequiringReadback(
            StopFailureKind.TimedOut,
            failure(VpnStopFailureCode.NotReady, "VPN owner stop timed out"),
            evidence,
        )
    } else {
        failedReceipt(
            StopFailureKind.Failed,
            failure(VpnStopFailureCode.InvalidState, "VPN timeout reply carried an unexpected status"),
            evidence,
        )
    }

    AndroidHostReply.VPN_STOP_SUPERSEDED -> if (statusCode == FfiErrorCode.INVALID_STATE.value.toInt()) {
        terminalReceiptRequiringReadback(
            StopFailureKind.Superseded,
            failure(VpnStopFailureCode.InvalidState, "VPN stop target was superseded"),
            evidence,
        )
    } else {
        failedReceipt(
            StopFailureKind.Failed,
            failure(VpnStopFailureCode.InvalidState, "VPN superseded reply carried an unexpected status"),
            evidence,
        )
    }

    AndroidHostReply.VPN_STOP_CANCELED -> if (statusCode == FfiErrorCode.NOT_READY.value.toInt()) {
        terminalReceiptRequiringReadback(
            StopFailureKind.Canceled,
            failure(VpnStopFailureCode.NotReady, "VPN stop request was canceled before readback"),
            evidence,
        )
    } else {
        failedReceipt(
            StopFailureKind.Failed,
            failure(VpnStopFailureCode.InvalidState, "VPN canceled reply carried an unexpected status"),
            evidence,
        )
    }

    AndroidHostReply.SERVICE_UNAVAILABLE -> failedReceipt(
        StopFailureKind.OwnerUnavailable,
        failure(VpnStopFailureCode.NotReady, "Android VPN owner is unavailable during stop"),
        evidence,
    )

    else -> failedReceipt(
        StopFailureKind.Failed,
        failure(VpnStopFailureCode.InvalidState, "Android VPN owner returned an unexpected stop reply"),
        evidence,
    )
}

private fun terminalReceiptRequiringReadback(
    kind: StopFailureKind,
    terminalFailure: VpnStopFailureWire,
    evidence: VpnStopReadbackEvidence,
): VpnOwnerStopReceiptWire = when (evidence) {
    VpnStopReadbackEvidence.Missing -> failedReceipt(
        StopFailureKind.Failed,
        failure(VpnStopFailureCode.NotReady, "VPN stop terminal omitted its owner-authored snapshot"),
        evidence,
    )

    is VpnStopReadbackEvidence.OwnerSnapshot -> failedReceipt(kind, terminalFailure, evidence)
}

private fun failedReceipt(kind: StopFailureKind, failure: VpnStopFailureWire, evidence: VpnStopReadbackEvidence): VpnOwnerStopReceiptWire =
    when (kind) {
        StopFailureKind.OwnerUnavailable -> when (evidence) {
            VpnStopReadbackEvidence.Missing -> VpnOwnerStopReceiptWire.OwnerUnavailable(failure)

            is VpnStopReadbackEvidence.OwnerSnapshot -> VpnOwnerStopReceiptWire.OwnerUnavailableWithReadback(
                failure,
                evidence.snapshot,
            )
        }

        StopFailureKind.Failed -> when (evidence) {
            VpnStopReadbackEvidence.Missing -> VpnOwnerStopReceiptWire.Failed(failure)

            is VpnStopReadbackEvidence.OwnerSnapshot -> VpnOwnerStopReceiptWire.FailedWithReadback(
                failure,
                evidence.snapshot,
            )
        }

        StopFailureKind.TimedOut -> when (evidence) {
            VpnStopReadbackEvidence.Missing -> VpnOwnerStopReceiptWire.TimedOut(failure)

            is VpnStopReadbackEvidence.OwnerSnapshot -> VpnOwnerStopReceiptWire.TimedOutWithReadback(
                failure,
                evidence.snapshot,
            )
        }

        StopFailureKind.Superseded -> when (evidence) {
            VpnStopReadbackEvidence.Missing -> VpnOwnerStopReceiptWire.Superseded(failure)

            is VpnStopReadbackEvidence.OwnerSnapshot -> VpnOwnerStopReceiptWire.SupersededWithReadback(
                failure,
                evidence.snapshot,
            )
        }

        StopFailureKind.Canceled -> when (evidence) {
            VpnStopReadbackEvidence.Missing -> VpnOwnerStopReceiptWire.Canceled(failure)

            is VpnStopReadbackEvidence.OwnerSnapshot -> VpnOwnerStopReceiptWire.CanceledWithReadback(
                failure,
                evidence.snapshot,
            )
        }
    }

private fun failure(code: VpnStopFailureCode, message: String) = VpnStopFailureWire(
    code = code,
    message = message,
    retryable = true,
)

package com.musicfrog.infiltrator

import infiltrator_android.AndroidHostReply
import infiltrator_android.AndroidOwnerDataDomain

internal typealias OwnerCoreSnapshot = CoreOwnerSnapshotWire

internal sealed interface OwnerCoreReadback {
    data object Unavailable : OwnerCoreReadback

    data class Available(val snapshot: OwnerCoreSnapshot) : OwnerCoreReadback
}

internal sealed interface OwnerOperationDataFacts {
    data object RevisionOnly : OwnerOperationDataFacts

    data class Domain(val domain: AndroidOwnerDataDomain) : OwnerOperationDataFacts

    data class Snapshot(val domain: AndroidOwnerDataDomain, val json: String) : OwnerOperationDataFacts
}

internal sealed interface OwnerDataReadback {
    data object Unavailable : OwnerDataReadback

    data class Available(val revision: Long, val generation: Long, val facts: OwnerOperationDataFacts) : OwnerDataReadback
}

internal enum class OwnerVpnAuthorization {
    Authorized,
    PermissionRequired,
}

internal fun OwnerVpnAuthorization.toAndroidHostReply(): AndroidHostReply = when (this) {
    OwnerVpnAuthorization.Authorized -> AndroidHostReply.VPN_AUTHORIZED
    OwnerVpnAuthorization.PermissionRequired -> AndroidHostReply.VPN_PERMISSION_REQUIRED
}

internal enum class OwnerCoreReadbackKind {
    Unavailable,
    Available,
}

internal enum class OwnerOperationDataFactsKind {
    RevisionOnly,
    Domain,
    Snapshot,
}

internal enum class OwnerResponsePayloadKind {
    Failure,
    CoreLifecycleSnapshot,
    VpnOwnerSnapshot,
    SurfaceSnapshot,
    OwnerDataRevision,
    Authorization,
    ProductCommand,
    OwnerOperation,
    StopVpn,
    VpnStart,
    VpnConfiguration,
    OwnerSessionReadback,
}

/** Every owner reply carries one legal, exhaustive payload variant. */
internal sealed interface OwnerResponsePayload {
    data object Failure : OwnerResponsePayload

    data class CoreLifecycleSnapshot(val snapshot: OwnerCoreSnapshot) : OwnerResponsePayload

    data class VpnOwnerSnapshot(val canonicalJson: String) : OwnerResponsePayload

    data class SurfaceSnapshot(val json: String, val coreReadback: OwnerCoreReadback) : OwnerResponsePayload

    data class OwnerDataRevision(val revision: Long, val generation: Long, val coreReadback: OwnerCoreReadback) : OwnerResponsePayload

    data class Authorization(val state: OwnerVpnAuthorization) : OwnerResponsePayload

    data class ProductCommand(val resultJson: String, val coreReadback: OwnerCoreReadback) : OwnerResponsePayload

    data class OwnerOperation(
        val outcomeJson: String,
        val revision: Long,
        val generation: Long,
        val dataFacts: OwnerOperationDataFacts,
        val coreReadback: OwnerCoreReadback,
    ) : OwnerResponsePayload

    data class StopVpn(val snapshotJson: String) : OwnerResponsePayload

    data class VpnStart(val outcome: VpnStartOutcomeWire) : OwnerResponsePayload

    data class VpnConfiguration(val outcome: VpnConfigurationApplyOutcomeWire) : OwnerResponsePayload

    data class OwnerSessionReadback(val ownerDataRevision: Long, val coreReadback: OwnerCoreReadback) : OwnerResponsePayload
}

internal fun OwnerResponsePayload.kind(): OwnerResponsePayloadKind = when (this) {
    OwnerResponsePayload.Failure -> OwnerResponsePayloadKind.Failure
    is OwnerResponsePayload.CoreLifecycleSnapshot -> OwnerResponsePayloadKind.CoreLifecycleSnapshot
    is OwnerResponsePayload.VpnOwnerSnapshot -> OwnerResponsePayloadKind.VpnOwnerSnapshot
    is OwnerResponsePayload.SurfaceSnapshot -> OwnerResponsePayloadKind.SurfaceSnapshot
    is OwnerResponsePayload.OwnerDataRevision -> OwnerResponsePayloadKind.OwnerDataRevision
    is OwnerResponsePayload.Authorization -> OwnerResponsePayloadKind.Authorization
    is OwnerResponsePayload.ProductCommand -> OwnerResponsePayloadKind.ProductCommand
    is OwnerResponsePayload.OwnerOperation -> OwnerResponsePayloadKind.OwnerOperation
    is OwnerResponsePayload.StopVpn -> OwnerResponsePayloadKind.StopVpn
    is OwnerResponsePayload.VpnStart -> OwnerResponsePayloadKind.VpnStart
    is OwnerResponsePayload.VpnConfiguration -> OwnerResponsePayloadKind.VpnConfiguration
    is OwnerResponsePayload.OwnerSessionReadback -> OwnerResponsePayloadKind.OwnerSessionReadback
}

internal fun OwnerResponsePayload.coreReadback(): OwnerCoreReadback = when (this) {
    OwnerResponsePayload.Failure,
    is OwnerResponsePayload.VpnOwnerSnapshot,
    is OwnerResponsePayload.Authorization,
    is OwnerResponsePayload.StopVpn,
    is OwnerResponsePayload.VpnStart,
    is OwnerResponsePayload.VpnConfiguration,
    -> OwnerCoreReadback.Unavailable

    is OwnerResponsePayload.CoreLifecycleSnapshot -> OwnerCoreReadback.Available(snapshot)

    is OwnerResponsePayload.SurfaceSnapshot -> coreReadback

    is OwnerResponsePayload.OwnerDataRevision -> coreReadback

    is OwnerResponsePayload.ProductCommand -> coreReadback

    is OwnerResponsePayload.OwnerOperation -> coreReadback

    is OwnerResponsePayload.OwnerSessionReadback -> coreReadback
}

internal fun OwnerResponsePayload.ownerDataReadback(): OwnerDataReadback = when (this) {
    OwnerResponsePayload.Failure,
    is OwnerResponsePayload.CoreLifecycleSnapshot,
    is OwnerResponsePayload.VpnOwnerSnapshot,
    is OwnerResponsePayload.SurfaceSnapshot,
    is OwnerResponsePayload.Authorization,
    is OwnerResponsePayload.ProductCommand,
    is OwnerResponsePayload.StopVpn,
    is OwnerResponsePayload.VpnStart,
    is OwnerResponsePayload.VpnConfiguration,
    -> OwnerDataReadback.Unavailable

    is OwnerResponsePayload.OwnerDataRevision -> OwnerDataReadback.Available(
        revision,
        generation,
        OwnerOperationDataFacts.RevisionOnly,
    )

    is OwnerResponsePayload.OwnerOperation -> OwnerDataReadback.Available(
        revision,
        generation,
        dataFacts,
    )

    is OwnerResponsePayload.OwnerSessionReadback -> OwnerDataReadback.Available(
        ownerDataRevision,
        (coreReadback as? OwnerCoreReadback.Available)?.snapshot?.generation ?: 0L,
        OwnerOperationDataFacts.RevisionOnly,
    )
}

internal fun OwnerResponsePayload.withCoreReadback(readback: OwnerCoreReadback): OwnerResponsePayload = when (this) {
    OwnerResponsePayload.Failure,
    is OwnerResponsePayload.CoreLifecycleSnapshot,
    is OwnerResponsePayload.VpnOwnerSnapshot,
    is OwnerResponsePayload.Authorization,
    is OwnerResponsePayload.StopVpn,
    is OwnerResponsePayload.VpnStart,
    is OwnerResponsePayload.VpnConfiguration,
    -> this

    is OwnerResponsePayload.SurfaceSnapshot -> copy(coreReadback = readback)

    is OwnerResponsePayload.OwnerDataRevision -> copy(
        generation = (readback as? OwnerCoreReadback.Available)?.snapshot?.generation ?: generation,
        coreReadback = readback,
    )

    is OwnerResponsePayload.ProductCommand -> copy(coreReadback = readback)

    is OwnerResponsePayload.OwnerOperation -> copy(
        generation = (readback as? OwnerCoreReadback.Available)?.snapshot?.generation ?: generation,
        coreReadback = readback,
    )

    is OwnerResponsePayload.OwnerSessionReadback -> copy(coreReadback = readback)
}

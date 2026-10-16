package com.musicfrog.infiltrator

import android.app.Service
import android.content.Intent
import android.net.VpnService
import android.os.Bundle
import android.os.Handler
import android.os.HandlerThread
import android.os.IBinder
import android.os.Message
import android.os.Messenger
import android.os.RemoteException
import android.os.SystemClock
import android.util.Log
import infiltrator_android.AndroidCoreLifecycleResult
import infiltrator_android.AndroidHostProtocolVersion
import infiltrator_android.AndroidHostReply
import infiltrator_android.AndroidHostRequest
import infiltrator_android.AndroidOwnerOperationIpcState
import infiltrator_android.FfiErrorCode
import infiltrator_android.androidBeginCoreOwnerSession
import infiltrator_android.androidCloseCoreOwner
import infiltrator_android.androidCoreLifecycleStatus
import infiltrator_android.androidDispatchOwnerOperationWireJson
import infiltrator_android.androidDispatchProfileAgeCredentialOwnerOperationWireJson
import infiltrator_android.androidOwnerDataRevision
import infiltrator_android.androidExecuteProductCommandJson
import infiltrator_android.androidSurfaceSnapshotJson
import kotlinx.coroutines.CoroutineStart
import kotlinx.coroutines.Job
import kotlinx.coroutines.launch
import java.util.concurrent.ArrayBlockingQueue
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.FutureTask
import java.util.concurrent.RejectedExecutionException
import java.util.concurrent.ThreadPoolExecutor
import java.util.concurrent.TimeUnit
import java.util.UUID

internal fun Bundle.putOwnerResponsePayload(payload: OwnerResponsePayload) {
    putSerializable(MihomoControlService.RESPONSE_PAYLOAD_KIND, payload.kind())
    when (payload) {
        OwnerResponsePayload.Failure -> Unit
        is OwnerResponsePayload.CoreLifecycleSnapshot -> putCoreSnapshot(payload.snapshot)
        is OwnerResponsePayload.VpnOwnerSnapshot -> putString(
            MihomoControlService.VPN_OWNER_SNAPSHOT_JSON,
            payload.canonicalJson,
        )
        is OwnerResponsePayload.SurfaceSnapshot -> {
            putString(MihomoControlService.SURFACE_SNAPSHOT_JSON, payload.json)
            putCoreReadback(payload.coreReadback)
        }
        is OwnerResponsePayload.OwnerDataRevision -> {
            putLong(MihomoControlService.OWNER_DATA_REVISION, payload.revision)
            putLong(MihomoControlService.OWNER_DATA_GENERATION, payload.generation)
            putCoreReadback(payload.coreReadback)
        }
        is OwnerResponsePayload.Authorization -> putSerializable(
            MihomoControlService.VPN_AUTHORIZATION_STATE,
            payload.state,
        )
        is OwnerResponsePayload.ProductCommand -> {
            putString(MihomoControlService.PRODUCT_COMMAND_RESULT_JSON, payload.resultJson)
            putCoreReadback(payload.coreReadback)
        }
        is OwnerResponsePayload.OwnerOperation -> {
            putString(MihomoControlService.OWNER_OPERATION_RESULT_JSON, payload.outcomeJson)
            putLong(MihomoControlService.OWNER_DATA_REVISION, payload.revision)
            putLong(MihomoControlService.OWNER_DATA_GENERATION, payload.generation)
            when (val facts = payload.dataFacts) {
                OwnerOperationDataFacts.RevisionOnly -> putSerializable(
                    MihomoControlService.OWNER_OPERATION_DATA_FACTS_KIND,
                    OwnerOperationDataFactsKind.RevisionOnly,
                )
                is OwnerOperationDataFacts.Domain -> {
                    putSerializable(
                        MihomoControlService.OWNER_OPERATION_DATA_FACTS_KIND,
                        OwnerOperationDataFactsKind.Domain,
                    )
                    putSerializable(MihomoControlService.OWNER_OPERATION_DATA_DOMAIN, facts.domain)
                }
                is OwnerOperationDataFacts.Snapshot -> {
                    putSerializable(
                        MihomoControlService.OWNER_OPERATION_DATA_FACTS_KIND,
                        OwnerOperationDataFactsKind.Snapshot,
                    )
                    putSerializable(MihomoControlService.OWNER_OPERATION_DATA_DOMAIN, facts.domain)
                    putString(MihomoControlService.OWNER_OPERATION_DATA_SNAPSHOT_JSON, facts.json)
                }
            }
            putCoreReadback(payload.coreReadback)
        }
        is OwnerResponsePayload.StopVpn -> putString(MihomoControlService.VPN_STOP_SNAPSHOT_JSON, payload.snapshotJson)
        is OwnerResponsePayload.VpnStart -> putSerializable(MihomoControlService.VPN_START_OUTCOME, payload.outcome)
        is OwnerResponsePayload.VpnConfiguration -> putSerializable(
            MihomoControlService.VPN_CONFIGURATION_OUTCOME,
            payload.outcome,
        )
        is OwnerResponsePayload.OwnerSessionReadback -> {
            putLong(MihomoControlService.OWNER_DATA_REVISION, payload.ownerDataRevision)
            putCoreReadback(payload.coreReadback)
        }
    }
}

internal fun Bundle.putCoreReadback(readback: OwnerCoreReadback) {
    when (readback) {
        OwnerCoreReadback.Unavailable -> putSerializable(
            MihomoControlService.CORE_READBACK_KIND,
            OwnerCoreReadbackKind.Unavailable,
        )
        is OwnerCoreReadback.Available -> {
            putSerializable(MihomoControlService.CORE_READBACK_KIND, OwnerCoreReadbackKind.Available)
            putCoreSnapshot(readback.snapshot)
        }
    }
}

internal fun Bundle.putCoreSnapshot(snapshot: OwnerCoreSnapshot) {
    putSerializable(MihomoControlService.CORE_SNAPSHOT_LIFECYCLE, snapshot.lifecycle)
    putLong(MihomoControlService.CORE_SNAPSHOT_GENERATION, snapshot.generation)
    putString(MihomoControlService.CORE_SNAPSHOT_SESSION_TOKEN, snapshot.sessionToken)
    putLong(MihomoControlService.CORE_SNAPSHOT_REVISION, snapshot.revision)
    putString(MihomoControlService.CORE_SNAPSHOT_IDENTITY_JSON, snapshot.identity.toJsonValue().toString())
    putString(MihomoControlService.CORE_SNAPSHOT_FAILURE_JSON, snapshot.failure?.toJson()?.toString())
}

internal suspend fun MihomoControlService.executeAdmitted(
    key: OwnerIpcRequestKey,
    request: AndroidHostRequest,
    arguments: Bundle,
    expectedOwnerSession: String?,
): ControlServiceResponse {
    try {
        if (expectedOwnerSession != null && expectedOwnerSession != ownerSessionId &&
            request != AndroidHostRequest.STOP_VPN_SERVICE &&
            request != AndroidHostRequest.READ_VPN_OWNER_SNAPSHOT
        ) {
            return recoverOwnerSession()
        }
        return execute(request, arguments, key)
    } finally {
        wipeOwnerRequestSensitivePayload(key)
    }
}

internal suspend fun MihomoControlService.execute(
    request: AndroidHostRequest,
    arguments: Bundle,
    requestKey: OwnerIpcRequestKey,
): ControlServiceResponse {
    val host = (application as? MihomoApplication)?.coreHost
        ?: return ControlServiceResponse(
            AndroidHostReply.SERVICE_UNAVAILABLE,
            FfiErrorCode.NOT_READY,
        )
    return when (request) {
        AndroidHostRequest.READ_CORE_LIFECYCLE -> readCoreLifecycle()
        AndroidHostRequest.EXECUTE_PRODUCT_COMMAND -> {
            val commandJson = arguments.getString(MihomoControlService.PRODUCT_COMMAND_JSON)
                ?: return ControlServiceResponse(AndroidHostReply.FAILED, FfiErrorCode.INVALID_INPUT)
            val resultJson = androidExecuteProductCommandJson(commandJson)
            ControlServiceResponse(
                AndroidHostReply.PRODUCT_COMMAND_COMPLETED,
                FfiErrorCode.OK,
                payload = OwnerResponsePayload.ProductCommand(
                    resultJson,
                    OwnerCoreReadback.Unavailable,
                ),
            ).withCurrentCoreSnapshot()
        }
        AndroidHostRequest.READ_SURFACE_SNAPSHOT -> {
            val result = androidSurfaceSnapshotJson()
            val snapshotJson = result.value
            if (result.status.code == FfiErrorCode.OK && snapshotJson != null) {
                ControlServiceResponse(
                    AndroidHostReply.SURFACE_SNAPSHOT_AVAILABLE,
                    FfiErrorCode.OK,
                    payload = OwnerResponsePayload.SurfaceSnapshot(
                        snapshotJson,
                        OwnerCoreReadback.Unavailable,
                    ),
                ).withCurrentCoreSnapshot()
            } else {
                ControlServiceResponse(
                    AndroidHostReply.FAILED,
                    result.status.code,
                    result.status.message,
                )
            }
        }
        AndroidHostRequest.READ_OWNER_DATA_REVISION ->
            ControlServiceResponse(
                AndroidHostReply.OWNER_DATA_REVISION_AVAILABLE,
                FfiErrorCode.OK,
                payload = OwnerResponsePayload.OwnerDataRevision(
                    androidOwnerDataRevision().toLong(),
                    generation = 0L,
                    coreReadback = OwnerCoreReadback.Unavailable,
                ),
            ).withCurrentCoreSnapshot()
        AndroidHostRequest.EXECUTE_OWNER_OPERATION -> {
            val operationJson = arguments.getString(MihomoControlService.OWNER_OPERATION_JSON)
                ?: return ControlServiceResponse(AndroidHostReply.FAILED, FfiErrorCode.INVALID_INPUT)
            val expectedOwnerSession = arguments.getString(MihomoControlService.EXPECTED_OWNER_SESSION_ID)
                ?: return ControlServiceResponse(AndroidHostReply.FAILED, FfiErrorCode.NOT_READY)
            val result = androidDispatchOwnerOperationWireJson(
                operationJson,
                arguments.getString(MihomoControlService.CLIENT_SESSION_ID).orEmpty(),
                expectedOwnerSession,
                ownerSessionId,
                arguments.getLong(MihomoControlService.CORE_GENERATION).toULong(),
                arguments.getLong(MihomoControlService.OWNER_DATA_REVISION).toULong(),
            )
            val reply = when (result.state) {
                AndroidOwnerOperationIpcState.COMPLETED -> AndroidHostReply.OWNER_OPERATION_COMPLETED
                AndroidOwnerOperationIpcState.STALE_OWNER_DATA -> AndroidHostReply.STALE_OWNER_DATA
                AndroidOwnerOperationIpcState.VPN_CONFIGURATION_BUSY -> AndroidHostReply.VPN_CONFIGURATION_BUSY
                AndroidOwnerOperationIpcState.FAILED -> AndroidHostReply.FAILED
            }
            val dataDomain = result.dataDomain
            val snapshotJson = result.snapshotJson
            ControlServiceResponse(
                reply = reply,
                statusCode = result.statusCode,
                statusMessage = result.statusMessage,
                payload = OwnerResponsePayload.OwnerOperation(
                    outcomeJson = result.outcomeJson,
                    revision = result.dataRevision.toLong(),
                    generation = result.coreGeneration.toLong(),
                    dataFacts = when {
                        dataDomain == null -> OwnerOperationDataFacts.RevisionOnly
                        snapshotJson == null -> OwnerOperationDataFacts.Domain(dataDomain)
                        else -> OwnerOperationDataFacts.Snapshot(dataDomain, snapshotJson)
                    },
                    coreReadback = OwnerCoreReadback.Unavailable,
                ),
            ).withCurrentCoreSnapshot()
        }
        AndroidHostRequest.EXECUTE_PROFILE_AGE_CREDENTIAL_OWNER_OPERATION -> {
            val metadataJson = runCatching {
                arguments.getString(MihomoControlService.PROFILE_AGE_CREDENTIAL_METADATA_JSON)
            }.getOrNull()
                ?: return ControlServiceResponse(AndroidHostReply.FAILED, FfiErrorCode.INVALID_INPUT)
            val privateKeyBytes = when (
                val payload = ownerRequestSensitivePayloads.lookup(requestKey)
            ) {
                OwnerIpcSensitivePayloadLookup.Missing -> return ControlServiceResponse(
                    AndroidHostReply.FAILED,
                    FfiErrorCode.INVALID_INPUT,
                    "Age credential request has no one-shot secret payload",
                )
                is OwnerIpcSensitivePayloadLookup.Present -> payload.bytes
            }
            val expectedOwnerSession = runCatching {
                arguments.getString(MihomoControlService.EXPECTED_OWNER_SESSION_ID)
            }.getOrNull()
                ?: run {
                    privateKeyBytes.fill(0)
                    return ControlServiceResponse(AndroidHostReply.FAILED, FfiErrorCode.NOT_READY)
                }
            val expectedCoreGeneration = runCatching {
                arguments.getLong(MihomoControlService.CORE_GENERATION, -1L)
            }.getOrDefault(-1L)
            val expectedOwnerDataRevision = runCatching {
                arguments.getLong(MihomoControlService.OWNER_DATA_REVISION, -1L)
            }.getOrDefault(-1L)
            if (expectedCoreGeneration < 0L || expectedOwnerDataRevision < 0L) {
                privateKeyBytes.fill(0)
                return ControlServiceResponse(
                    AndroidHostReply.FAILED,
                    FfiErrorCode.INVALID_INPUT,
                    "Age credential request omitted its owner revision fence",
                )
            }
            val result = try {
                androidDispatchProfileAgeCredentialOwnerOperationWireJson(
                    metadataJson,
                    privateKeyBytes,
                    runCatching { arguments.getString(MihomoControlService.CLIENT_SESSION_ID) }
                        .getOrNull().orEmpty(),
                    expectedOwnerSession,
                    ownerSessionId,
                    expectedCoreGeneration.toULong(),
                    expectedOwnerDataRevision.toULong(),
                )
            } finally {
                privateKeyBytes.fill(0)
            }
            val reply = when (result.state) {
                AndroidOwnerOperationIpcState.COMPLETED -> AndroidHostReply.OWNER_OPERATION_COMPLETED
                AndroidOwnerOperationIpcState.STALE_OWNER_DATA -> AndroidHostReply.STALE_OWNER_DATA
                AndroidOwnerOperationIpcState.VPN_CONFIGURATION_BUSY -> AndroidHostReply.VPN_CONFIGURATION_BUSY
                AndroidOwnerOperationIpcState.FAILED -> AndroidHostReply.FAILED
            }
            val dataDomain = result.dataDomain
            val snapshotJson = result.snapshotJson
            ControlServiceResponse(
                reply = reply,
                statusCode = result.statusCode,
                statusMessage = result.statusMessage,
                payload = OwnerResponsePayload.OwnerOperation(
                    outcomeJson = result.outcomeJson,
                    revision = result.dataRevision.toLong(),
                    generation = result.coreGeneration.toLong(),
                    dataFacts = when {
                        dataDomain == null -> OwnerOperationDataFacts.RevisionOnly
                        snapshotJson == null -> OwnerOperationDataFacts.Domain(dataDomain)
                        else -> OwnerOperationDataFacts.Snapshot(dataDomain, snapshotJson)
                    },
                    coreReadback = OwnerCoreReadback.Unavailable,
                ),
            ).withCurrentCoreSnapshot()
        }
        AndroidHostRequest.START_VPN_SERVICE -> ControlServiceResponse(
            AndroidHostReply.SERVICE_UNAVAILABLE,
            FfiErrorCode.NOT_READY,
            "VPN start must use bounded owner admission",
        )
        AndroidHostRequest.STOP_VPN_SERVICE -> stopVpn(arguments)
        AndroidHostRequest.READ_VPN_AUTHORIZATION -> {
            val authorization = readVpnAuthorization()
            ControlServiceResponse(
                reply = authorization.toAndroidHostReply(),
                statusCode = FfiErrorCode.OK,
                payload = OwnerResponsePayload.Authorization(authorization),
            )
        }
        AndroidHostRequest.READ_VPN_OWNER_SNAPSHOT -> readVpnOwnerSnapshot()
        AndroidHostRequest.APPLY_VPN_CONFIGURATION ->
            applyVpnConfiguration(arguments)
    }
}

internal suspend fun MihomoControlService.readVpnOwnerSnapshot(): ControlServiceResponse {
    val snapshot = currentVpnOwnerSnapshotJson()
        ?: return ControlServiceResponse(
            AndroidHostReply.SERVICE_UNAVAILABLE,
            FfiErrorCode.NOT_READY,
            "VPN owner snapshot is unavailable",
        )
    return ControlServiceResponse(
        reply = AndroidHostReply.VPN_OWNER_SNAPSHOT_AVAILABLE,
        statusCode = FfiErrorCode.OK,
        payload = OwnerResponsePayload.VpnOwnerSnapshot(snapshot),
    )
}

internal suspend fun MihomoControlService.stopVpn(arguments: Bundle): ControlServiceResponse {
    val deadline = readRequestDeadline(arguments)
    if (deadline == null) {
        val snapshot = currentVpnOwnerSnapshotJson()
        return ControlServiceResponse(
            reply = AndroidHostReply.VPN_STOP_TIMED_OUT,
            statusCode = FfiErrorCode.NOT_READY,
            statusMessage = "VPN stop request omitted its absolute deadline",
            payload = snapshot?.let { OwnerResponsePayload.StopVpn(it) }
                ?: OwnerResponsePayload.Failure,
        )
    }
    if (!arguments.containsKey(MihomoControlService.VPN_STOP_TARGET_CODE) ||
        !arguments.containsKey(MihomoControlService.VPN_STOP_GENERATION)
    ) {
        val snapshot = currentVpnOwnerSnapshotJson()
        return ControlServiceResponse(
            AndroidHostReply.FAILED,
            FfiErrorCode.INVALID_INPUT,
            "VPN stop request omitted its typed target",
            payload = snapshot?.let { OwnerResponsePayload.StopVpn(it) }
                ?: OwnerResponsePayload.Failure,
        )
    }
    val target = VpnStopTarget.fromWire(
        arguments.getInt(MihomoControlService.VPN_STOP_TARGET_CODE),
        arguments.getLong(MihomoControlService.VPN_STOP_GENERATION),
    ) ?: return ControlServiceResponse(
        reply = AndroidHostReply.FAILED,
        statusCode = FfiErrorCode.INVALID_INPUT,
        statusMessage = "VPN stop request carried an invalid target",
        payload = currentVpnOwnerSnapshotJson()?.let { OwnerResponsePayload.StopVpn(it) }
            ?: OwnerResponsePayload.Failure,
    )
    when (target) {
        VpnStopTarget.Current -> Unit
        is VpnStopTarget.Generation -> return ControlServiceResponse(
            reply = AndroidHostReply.FAILED,
            statusCode = FfiErrorCode.INVALID_INPUT,
            statusMessage = "Generation VPN stop targets are owner-local worker callbacks",
            payload = currentVpnOwnerSnapshotJson()?.let { OwnerResponsePayload.StopVpn(it) }
                ?: OwnerResponsePayload.Failure,
        )
    }
    val result = MihomoVpnService.stopAndAwaitStopped(this, target, deadline)
    val snapshotJson = currentVpnOwnerSnapshotJson()
    val snapshot = snapshotJson?.let(VpnOwnerSnapshotWire::parse)
    val response = when (result) {
        VpnStopResult.Stopped -> when (snapshot?.isTerminalStopped()) {
            true -> ControlServiceResponse(
                AndroidHostReply.VPN_STOPPED,
                FfiErrorCode.OK,
            )
            false, null -> ControlServiceResponse(
                AndroidHostReply.VPN_STOP_SUPERSEDED,
                FfiErrorCode.INVALID_STATE,
                "VPN stop was superseded before owner readback",
            )
        }
        is VpnStopResult.Failed -> {
            Log.e(MihomoControlService.TAG, "VpnService shutdown failed: ${result.message}")
            ControlServiceResponse(
                AndroidHostReply.FAILED,
                FfiErrorCode.INVALID_STATE,
                result.message,
            )
        }
        VpnStopResult.TimedOut -> ControlServiceResponse(
            AndroidHostReply.VPN_STOP_TIMED_OUT,
            FfiErrorCode.NOT_READY,
            "VpnService did not stop before the shutdown deadline",
        )
        VpnStopResult.Superseded -> ControlServiceResponse(
            AndroidHostReply.VPN_STOP_SUPERSEDED,
            FfiErrorCode.INVALID_STATE,
            "VPN stop request belongs to an older generation",
        )
        VpnStopResult.Canceled -> ControlServiceResponse(
            AndroidHostReply.VPN_STOP_CANCELED,
            FfiErrorCode.NOT_READY,
            "VPN stop wait was canceled before owner readback",
        )
    }
    return if (snapshot == null || snapshotJson == null) {
        response.copy(
            reply = AndroidHostReply.SERVICE_UNAVAILABLE,
            statusCode = FfiErrorCode.NOT_READY,
            statusMessage = "VPN stop terminal omitted its owner-authored snapshot",
            payload = OwnerResponsePayload.Failure,
        )
    } else {
        response.copy(payload = OwnerResponsePayload.StopVpn(snapshotJson))
    }
}

internal suspend fun MihomoControlService.currentVpnOwnerSnapshotJson(): String? =
    (application as? MihomoApplication)?.refreshAndPublishVpnOwnerSnapshotJson()

/**
 * A request carrying the retired process session is rejected. The client
 * treats this Core-only reply as partial and obtains one complete VPN/Core
 * owner snapshot before it issues another operation.
 */
internal suspend fun MihomoControlService.recoverOwnerSession(): ControlServiceResponse {
    val lifecycle = readCoreLifecycle()
    return lifecycle.copy(
        reply = AndroidHostReply.OWNER_SESSION_CHANGED,
        statusMessage = lifecycle.statusMessage
            ?: "Android :vpn owner changed; lifecycle state was refreshed",
        payload = OwnerResponsePayload.OwnerSessionReadback(
            ownerDataRevision = androidOwnerDataRevision().toLong(),
            coreReadback = lifecycle.payload.coreReadback(),
        ),
    )
}

internal suspend fun ControlServiceResponse.withCurrentCoreSnapshot(): ControlServiceResponse {
    val readback = androidCoreLifecycleStatus().toControlOwnerCoreReadback()
    return copy(payload = payload.withCoreReadback(readback))
}

internal suspend fun MihomoControlService.readCoreLifecycle(): ControlServiceResponse {
    val result = androidCoreLifecycleStatus()
    return when {
        result.status.code != FfiErrorCode.OK -> result.toControlServiceResponse(AndroidHostReply.FAILED)
        else -> when (val core = result.toControlOwnerCoreReadback()) {
            OwnerCoreReadback.Unavailable -> ControlServiceResponse(
                AndroidHostReply.FAILED,
                FfiErrorCode.NOT_READY,
                "Core lifecycle snapshot is unavailable",
            )
            is OwnerCoreReadback.Available -> ControlServiceResponse(
                AndroidHostReply.CORE_LIFECYCLE_AVAILABLE,
                FfiErrorCode.OK,
                payload = OwnerResponsePayload.CoreLifecycleSnapshot(core.snapshot),
            )
        }
    }
}

internal fun MihomoControlService.applyVpnConfiguration(arguments: Bundle): ControlServiceResponse {
    val configuration = arguments.getString(MihomoControlService.VPN_CONFIGURATION)
    if (configuration == null) {
        return ControlServiceResponse(AndroidHostReply.FAILED, FfiErrorCode.INVALID_INPUT)
    }
    return when (MihomoVpnService.setPendingConfiguration(this, configuration)) {
        VpnConfigurationApplyOutcomeWire.Accepted ->
            ControlServiceResponse(
                AndroidHostReply.VPN_CONFIGURATION_ACCEPTED,
                FfiErrorCode.OK,
                payload = OwnerResponsePayload.VpnConfiguration(
                    VpnConfigurationApplyOutcomeWire.Accepted,
                ),
            )
        VpnConfigurationApplyOutcomeWire.Rejected ->
            ControlServiceResponse(
                AndroidHostReply.FAILED,
                FfiErrorCode.INVALID_INPUT,
                payload = OwnerResponsePayload.VpnConfiguration(
                    VpnConfigurationApplyOutcomeWire.Rejected,
                ),
            )
        VpnConfigurationApplyOutcomeWire.OwnerUnavailable ->
            ControlServiceResponse(
                AndroidHostReply.SERVICE_UNAVAILABLE,
                FfiErrorCode.NOT_READY,
                payload = OwnerResponsePayload.VpnConfiguration(
                    VpnConfigurationApplyOutcomeWire.OwnerUnavailable,
                ),
            )
    }
}

internal fun MihomoControlService.startVpnService(
    key: VpnOwnerRequestKey,
    deadline: VpnOwnerRequestDeadline,
): ControlServiceResponse {
    when (val authorization = readVpnAuthorization()) {
        OwnerVpnAuthorization.PermissionRequired -> {
            VpnAuthorizationBroadcast.requestForExplicitStart(this)
            return ControlServiceResponse(
                AndroidHostReply.VPN_PERMISSION_REQUIRED,
                FfiErrorCode.OK,
                payload = OwnerResponsePayload.VpnStart(VpnStartOutcomeWire.PermissionRequired),
            )
        }
        OwnerVpnAuthorization.Authorized -> Unit
    }
    val result = MihomoVpnService.startAndAwaitRunning(
        context = this,
        deadline = deadline,
        generationSelected = { generation ->
            vpnStartAdmission.associateGeneration(key, generation)
        },
        generationAlreadyExecuting = { generation ->
            vpnStartAdmission.associateAlreadyExecutingGeneration(key, generation)
        },
        generationDispatchPending = { generation ->
            vpnStartAdmission.markServiceDispatchPending(key, generation)
        },
        generationDispatchStarted = { generation ->
            vpnStartAdmission.markServiceDispatchStarted(key, generation)
        },
        generationDispatched = { generation ->
            vpnStartAdmission.markServiceDispatched(key, generation)
        },
    )
    val outcome = result.toWireOutcome()
    return when (result) {
        VpnStartResult.Running -> ControlServiceResponse(
            AndroidHostReply.VPN_RUNNING,
            FfiErrorCode.OK,
            payload = OwnerResponsePayload.VpnStart(outcome),
        )
        VpnStartResult.PermissionRequired -> ControlServiceResponse(
            AndroidHostReply.VPN_PERMISSION_REQUIRED,
            FfiErrorCode.OK,
            payload = OwnerResponsePayload.VpnStart(outcome),
        )
        is VpnStartResult.Failed -> {
            Log.e(MihomoControlService.TAG, "VpnService startup failed: ${result.message}")
            ControlServiceResponse(
                AndroidHostReply.FAILED,
                FfiErrorCode.INVALID_STATE,
                result.message,
                OwnerResponsePayload.VpnStart(outcome),
            )
        }
        VpnStartResult.TimedOut -> {
            Log.e(MihomoControlService.TAG, "VpnService startup timed out before proxy data path was running")
            ControlServiceResponse(
                AndroidHostReply.VPN_START_TIMED_OUT,
                FfiErrorCode.NOT_READY,
                payload = OwnerResponsePayload.VpnStart(outcome),
            )
        }
    }
}

internal fun MihomoControlService.readVpnAuthorization(): OwnerVpnAuthorization {
    return when (VpnService.prepare(this)) {
        null -> OwnerVpnAuthorization.Authorized
        else -> OwnerVpnAuthorization.PermissionRequired
    }
}

internal fun MihomoControlService.readRequest(bundle: Bundle): AndroidHostRequest? {
    bundle.classLoader = AndroidHostRequest::class.java.classLoader
    return if (android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.TIRAMISU) {
        bundle.getSerializable(MihomoControlService.REQUEST_VALUE, AndroidHostRequest::class.java)
    } else {
        @Suppress("DEPRECATION")
        bundle.getSerializable(MihomoControlService.REQUEST_VALUE) as? AndroidHostRequest
    }
}

internal fun MihomoControlService.readProtocolVersion(bundle: Bundle): AndroidHostProtocolVersion? {
    bundle.classLoader = AndroidHostProtocolVersion::class.java.classLoader
    return if (android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.TIRAMISU) {
        bundle.getSerializable(MihomoControlService.PROTOCOL_VERSION, AndroidHostProtocolVersion::class.java)
    } else {
        @Suppress("DEPRECATION")
        bundle.getSerializable(MihomoControlService.PROTOCOL_VERSION) as? AndroidHostProtocolVersion
    }
}

internal fun MihomoControlService.readRequestDeadline(bundle: Bundle): VpnOwnerRequestDeadline? {
    if (!bundle.containsKey(MihomoControlService.REQUEST_DEADLINE_ELAPSED_REALTIME)) return null
    val deadline = bundle.getLong(MihomoControlService.REQUEST_DEADLINE_ELAPSED_REALTIME, 0L)
    return deadline.takeIf { it > 0L }?.let(::VpnOwnerRequestDeadline)
}

internal fun AndroidCoreLifecycleResult.toControlServiceResponse(accepted: AndroidHostReply): ControlServiceResponse =
    when (val core = toControlOwnerCoreReadback()) {
        OwnerCoreReadback.Unavailable -> ControlServiceResponse(
            accepted,
            status.code,
            status.message,
        )
        is OwnerCoreReadback.Available -> ControlServiceResponse(
            accepted,
            status.code,
            status.message,
            OwnerResponsePayload.CoreLifecycleSnapshot(core.snapshot),
        )
    }

internal fun AndroidCoreLifecycleResult.toControlOwnerCoreReadback(): OwnerCoreReadback {
    if (status.code != FfiErrorCode.OK) return OwnerCoreReadback.Unavailable
    val state = lifecycle?.toCoreOwnerLifecycleWire() ?: return OwnerCoreReadback.Unavailable
    val ownerGeneration = generation.toLong()
    val ownerRevision = revision.toLong()
    val ownerSessionToken = sessionToken
    val invalidSessionToken = ownerSessionToken?.let { token ->
        token.isBlank() || !token.all(Char::isDigit)
    } == true
    if (ownerGeneration < 0L || ownerRevision < 0L || invalidSessionToken) {
        return OwnerCoreReadback.Unavailable
    }
    val identity = CoreOwnerIdentityWire.fromNullableParts(engineId, coreInstanceId, coreVersion)
        ?: return OwnerCoreReadback.Unavailable
    val failure = when (val rawFailure = failureJson) {
        null -> null
        else -> CoreOwnerFailureWire.parse(rawFailure) ?: return OwnerCoreReadback.Unavailable
    }
    return OwnerCoreReadback.Available(
        OwnerCoreSnapshot(
            lifecycle = state,
            generation = ownerGeneration,
            sessionToken = ownerSessionToken,
            revision = ownerRevision,
            identity = identity,
            failure = failure,
        ),
    )
}

internal fun infiltrator_android.FfiStatus.toControlServiceResponse(accepted: AndroidHostReply): ControlServiceResponse =
    ControlServiceResponse(
        if (code == FfiErrorCode.OK) accepted else AndroidHostReply.FAILED,
        code,
        message,
    )

internal data class ControlServiceResponse(
    val reply: AndroidHostReply,
    val statusCode: FfiErrorCode,
    val statusMessage: String? = null,
    val payload: OwnerResponsePayload = OwnerResponsePayload.Failure,
)

internal enum class OwnerIpcWaiterAdmission {
    Attached,
    CapacityExceeded,
    NoActiveRequest,
}

internal enum class VpnStartReplyAdmission {
    Attached,
    CapacityExceeded,
    NoActiveRequest,
}


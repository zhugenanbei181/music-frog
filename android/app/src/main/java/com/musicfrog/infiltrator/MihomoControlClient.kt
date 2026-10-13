package com.musicfrog.infiltrator

import android.content.Context
import android.os.Bundle
import android.os.Message
import android.os.Messenger
import android.os.RemoteException
import android.os.SystemClock
import android.util.Log
import infiltrator_android.AndroidHostProtocolVersion
import infiltrator_android.AndroidHostReply
import infiltrator_android.AndroidOwnerDataDomain
import infiltrator_android.FfiErrorCode
import java.util.UUID
import java.util.concurrent.atomic.AtomicLong

/** Typed synchronous client for the manifest-declared `:vpn` process owner. */
internal class MihomoControlClient(context: Context) {
    private val applicationContext = context.applicationContext
    private val clientSessionId = UUID.randomUUID().toString()
    private val ownerSessionFence = AndroidOwnerSessionFence()
    private val coreSnapshotFence = AndroidCoreSnapshotFence()
    private val vpnSnapshotFence = AndroidVpnSnapshotFence()
    private val ownerDataFence = AndroidOwnerDataFence()
    private val ipcConnection = (applicationContext as? MihomoApplication)?.ownerIpcConnection
        ?: throw IllegalStateException("MihomoApplication must own the Android Binder connection")

    fun startVpnService(
        deadline: VpnOwnerRequestDeadline = newRequestDeadline(),
    ): VpnStartOutcomeWire {
        val response = transact(OwnerIpcPayload.StartVpnService, deadline)
        return when (response.terminal) {
            OwnerClientTerminal.ReplyTimedOut -> VpnStartOutcomeWire.TimedOut
            OwnerClientTerminal.Interrupted -> VpnStartOutcomeWire.Failed
            else -> (response.payload as? OwnerResponsePayload.VpnStart)?.outcome
                ?: response.reply.toVpnStartOutcomeWire()
        }
    }

    fun stopVpnService(
        deadline: VpnOwnerRequestDeadline = newRequestDeadline(),
    ): String? {
        val response = transact(OwnerIpcPayload.StopVpnService(VpnStopTarget.Current), deadline)
        if (response.terminal in setOf(
                OwnerClientTerminal.OwnerUnavailable,
                OwnerClientTerminal.BindRejected,
                OwnerClientTerminal.NullBinding,
                OwnerClientTerminal.BinderDied,
                OwnerClientTerminal.BindingDied,
                OwnerClientTerminal.SendFailed,
                OwnerClientTerminal.PendingCapacityExceeded,
            )
        ) {
            return vpnOwnerStopReceiptJson(
                VpnOwnerStopReceiptWire.OwnerUnavailable(
                    VpnStopFailureWire(
                        code = VpnStopFailureCode.NotReady,
                        message = "Android VPN owner is unavailable during stop",
                        retryable = true,
                    ),
                ),
            )
        }
        val snapshot = (response.payload as? OwnerResponsePayload.StopVpn)
            ?.snapshotJson
            ?.let(VpnOwnerSnapshotWire::parse)
        return VpnStopOwnerReadback(
            reply = response.reply,
            statusCode = response.statusCode,
            evidence = when (snapshot) {
                null -> VpnStopReadbackEvidence.Missing
                else -> VpnStopReadbackEvidence.OwnerSnapshot(snapshot)
            },
            terminal = response.terminal,
        ).toReceiptJson()
    }

    fun applyVpnConfiguration(
        configJson: String,
        deadline: VpnOwnerRequestDeadline = newRequestDeadline(),
    ): VpnConfigurationApplyOutcomeWire {
        val response = transact(OwnerIpcPayload.ApplyVpnConfiguration(configJson), deadline)
        return when (val payload = response.payload) {
            is OwnerResponsePayload.VpnConfiguration -> payload.outcome
            else -> response.reply.toVpnConfigurationApplyOutcomeWire()
        }
    }

    fun executeProductCommandJson(
        commandJson: String,
        deadline: VpnOwnerRequestDeadline = newRequestDeadline(),
    ): String? {
        val response = transact(OwnerIpcPayload.ExecuteProductCommand(commandJson), deadline)
        val payload = response.payload as? OwnerResponsePayload.ProductCommand
        return payload?.resultJson?.takeIf {
            response.reply == AndroidHostReply.PRODUCT_COMMAND_COMPLETED &&
                response.statusCode == FfiErrorCode.OK.value.toInt()
        }
    }

    fun executeOwnerOperationJson(
        operationJson: String,
        deadline: VpnOwnerRequestDeadline = newRequestDeadline(),
    ): String? {
        val stamp = ensureOwnerDataFence(deadline) ?: return null
        val response = transact(
            OwnerIpcPayload.ExecuteOwnerOperation(
                operationJson = operationJson,
                expectedCoreGeneration = stamp.coreGeneration,
                expectedOwnerDataRevision = stamp.revision,
            ),
            deadline,
        )
        val payload = response.payload as? OwnerResponsePayload.OwnerOperation
        return payload?.outcomeJson?.takeIf {
            response.reply in setOf(
                AndroidHostReply.OWNER_OPERATION_COMPLETED,
                AndroidHostReply.VPN_CONFIGURATION_BUSY,
                AndroidHostReply.STALE_OWNER_DATA,
                AndroidHostReply.FAILED,
            )
        }
    }

    private fun ensureOwnerDataFence(deadline: VpnOwnerRequestDeadline): OwnerDataStamp? {
        val ownerSession = ownerSessionFence.current()
        val current = ownerDataFence.current()
        if (current != null && current.ownerSessionId == ownerSession) return current

        val response = transact(OwnerIpcPayload.OwnerDataRevisionReadback, deadline)
        if (response.reply != AndroidHostReply.OWNER_DATA_REVISION_AVAILABLE &&
            response.reply != AndroidHostReply.OWNER_SESSION_CHANGED
        ) {
            return null
        }
        return ownerDataFence.current()?.takeIf { it.ownerSessionId == ownerSessionFence.current() }
    }

    fun readSurfaceSnapshotJson(
        deadline: VpnOwnerRequestDeadline = newRequestDeadline(),
    ): String? {
        val response = transact(OwnerIpcPayload.SurfaceSnapshotReadback, deadline)
        val payload = response.payload as? OwnerResponsePayload.SurfaceSnapshot
        return payload?.json?.takeIf {
            response.reply == AndroidHostReply.SURFACE_SNAPSHOT_AVAILABLE &&
                response.statusCode == FfiErrorCode.OK.value.toInt()
        }
    }

    fun readCoreLifecycleSnapshotJson(
        deadline: VpnOwnerRequestDeadline = newRequestDeadline(),
    ): String? {
        val response = transact(OwnerIpcPayload.VpnOwnerSnapshotReadback, deadline)
        if (response.statusCode != FfiErrorCode.OK.value.toInt()) return null
        return (response.payload as? OwnerResponsePayload.VpnOwnerSnapshot)
            ?.canonicalJson
            ?.let(VpnOwnerSnapshotWire::parse)
            ?.coreLifecycleJson()
    }

    fun readVpnOwnerSnapshotJson(
        deadline: VpnOwnerRequestDeadline = newRequestDeadline(),
    ): String? = readVpnOwnerSnapshot(deadline)?.canonicalJson

    fun readVpnOwnerSnapshot(
        deadline: VpnOwnerRequestDeadline = newRequestDeadline(),
    ): VpnOwnerSnapshotWire? {
        val response = transact(OwnerIpcPayload.VpnOwnerSnapshotReadback, deadline)
        val snapshot = (response.payload as? OwnerResponsePayload.VpnOwnerSnapshot)
            ?.canonicalJson
            ?.let(VpnOwnerSnapshotWire::parse)
        return snapshot.takeIf {
            response.reply == AndroidHostReply.VPN_OWNER_SNAPSHOT_AVAILABLE &&
                response.statusCode == FfiErrorCode.OK.value.toInt()
        }
    }

    fun readVpnAuthorization(
        deadline: VpnOwnerRequestDeadline = newRequestDeadline(),
    ): AndroidHostReply =
        when (val payload = transact(OwnerIpcPayload.VpnAuthorizationReadback, deadline).payload) {
            is OwnerResponsePayload.Authorization -> payload.state.toAndroidHostReply()
            else -> AndroidHostReply.SERVICE_UNAVAILABLE
        }

    private fun transact(
        payload: OwnerIpcPayload,
        deadline: VpnOwnerRequestDeadline = newRequestDeadline(),
    ): OwnerResponse {
        if (payload !is OwnerIpcPayload.VpnOwnerSnapshotReadback &&
            payload !is OwnerIpcPayload.StopVpnService &&
            ownerSessionFence.readiness() == OwnerSessionReadiness.ReadbackRequired
        ) {
            val ownerReadback = transactBound(OwnerIpcPayload.VpnOwnerSnapshotReadback, deadline)
            if (ownerSessionFence.readiness() != OwnerSessionReadiness.Ready) {
                return if (ownerReadback.terminal == OwnerClientTerminal.ReplyReceived) {
                    ownerReadback.copy(terminal = OwnerClientTerminal.OwnerSessionReadbackRequired)
                } else {
                    ownerReadback
                }
            }
            if (payload is OwnerIpcPayload.VpnOwnerSnapshotReadback) return ownerReadback
        }
        return transactBound(payload, deadline)
    }

    private fun transactBound(
        payload: OwnerIpcPayload,
        deadline: VpnOwnerRequestDeadline,
    ): OwnerResponse {
        if (payload.role == OwnerIpcRequestRole.VpnStop && payload !is OwnerIpcPayload.StopVpnService) {
            return OwnerResponse.unavailable(OwnerClientTerminal.InvalidReply)
        }
        val request = payload.request
        if (deadline.remainingAt(SystemClock.elapsedRealtime()) == VpnDeadlineState.Expired) {
            return OwnerResponse.unavailable(OwnerClientTerminal.ReplyTimedOut)
        }
        val requestId = REQUEST_IDS.getAndIncrement()
        val expectedOwnerSession = ownerSessionFence.current()
        val requestMessage = Message.obtain(null, MihomoControlService.REQUEST).apply {
            data = Bundle().apply {
                putSerializable(
                    MihomoControlService.PROTOCOL_VERSION,
                    AndroidHostProtocolVersion.V9,
                )
                putString(MihomoControlService.CLIENT_SESSION_ID, clientSessionId)
                putString(MihomoControlService.EXPECTED_OWNER_SESSION_ID, expectedOwnerSession)
                putLong(MihomoControlService.REQUEST_ID, requestId)
                putLong(
                    MihomoControlService.REQUEST_DEADLINE_ELAPSED_REALTIME,
                    deadline.elapsedRealtimeMillis,
                )
                putSerializable(MihomoControlService.REQUEST_VALUE, request)
                when (payload) {
                    OwnerIpcPayload.CoreLifecycleReadback,
                    OwnerIpcPayload.VpnOwnerSnapshotReadback,
                    OwnerIpcPayload.SurfaceSnapshotReadback,
                    OwnerIpcPayload.OwnerDataRevisionReadback,
                    OwnerIpcPayload.VpnAuthorizationReadback,
                    OwnerIpcPayload.StartVpnService -> Unit
                    is OwnerIpcPayload.StopVpnService -> when (val target = payload.target) {
                        VpnStopTarget.Current -> {
                            putInt(MihomoControlService.VPN_STOP_TARGET_CODE, 0)
                            putLong(MihomoControlService.VPN_STOP_GENERATION, 0L)
                        }
                        is VpnStopTarget.Generation -> {
                            putInt(MihomoControlService.VPN_STOP_TARGET_CODE, 1)
                            putLong(
                                MihomoControlService.VPN_STOP_GENERATION,
                                target.value,
                            )
                        }
                    }
                    is OwnerIpcPayload.ApplyVpnConfiguration ->
                        putString(MihomoControlService.VPN_CONFIGURATION, payload.configurationJson)
                    is OwnerIpcPayload.ExecuteProductCommand ->
                        putString(MihomoControlService.PRODUCT_COMMAND_JSON, payload.commandJson)
                    is OwnerIpcPayload.ExecuteOwnerOperation -> {
                        putString(MihomoControlService.OWNER_OPERATION_JSON, payload.operationJson)
                        putLong(MihomoControlService.CORE_GENERATION, payload.expectedCoreGeneration)
                        putLong(
                            MihomoControlService.OWNER_DATA_REVISION,
                            payload.expectedOwnerDataRevision,
                        )
                    }
                }
            }
        }
        fun unavailable(terminal: OwnerClientTerminal): OwnerResponse =
            when (ownerSessionFence.requireReadback(requestId, expectedOwnerSession, terminal)) {
                OwnerSessionInvalidation.ReadbackRequired -> OwnerResponse.unavailable(terminal)
                OwnerSessionInvalidation.RetiredBinding -> {
                    val currentSession = ownerSessionFence.current()
                    if (currentSession == null) {
                        OwnerResponse.unavailable(OwnerClientTerminal.OwnerSessionStale)
                    } else {
                        OwnerResponse.staleOwnerSession(currentSession)
                    }
                }
            }
        val exchange = ipcConnection.transact(
            request = request,
            requestId = requestId,
            deadline = deadline,
            message = requestMessage,
            decodeReply = { response ->
                if (response.what != MihomoControlService.RESPONSE) {
                    unavailable(OwnerClientTerminal.InvalidReply)
                } else {
                    val responseData = runCatching { response.data }.getOrNull()
                    if (responseData == null) {
                        unavailable(OwnerClientTerminal.InvalidReply)
                    } else {
                        val receivedRequestId = runCatching {
                            responseData.getLong(MihomoControlService.REQUEST_ID, Long.MIN_VALUE)
                        }.getOrDefault(Long.MIN_VALUE)
                        if (receivedRequestId != requestId) {
                            unavailable(OwnerClientTerminal.OwnerSessionStale)
                        } else {
                            runCatching {
                                readResponse(
                                    responseData,
                                    expectedOwnerSession,
                                    requestId,
                                    payload.role,
                                )
                            }.getOrElse { error ->
                                Log.w(TAG, "owner IPC reply was invalid: ${error.message}")
                                unavailable(OwnerClientTerminal.InvalidReply)
                            }
                        }
                    }
                }
            },
            shouldAcknowledge = { response ->
                response.terminal != OwnerClientTerminal.InvalidReply &&
                    deadline.remainingAt(SystemClock.elapsedRealtime()) != VpnDeadlineState.Expired
            },
            onFailure = ::unavailable,
            onCancel = { messenger -> sendCancellation(request, requestId, messenger) },
            onAcknowledge = { messenger -> acknowledgeOwnerStartRequest(requestId, messenger) },
        )
        return when (exchange) {
            is OwnerIpcTransportResult.Completed -> exchange.response
            OwnerIpcTransportResult.PendingCapacityExceeded ->
                unavailable(OwnerClientTerminal.PendingCapacityExceeded)
            OwnerIpcTransportResult.RequestIdAlreadyPending ->
                unavailable(OwnerClientTerminal.InvalidReply)
        }
    }

    private fun readResponse(
        bundle: Bundle,
        expectedOwnerSession: String?,
        requestId: Long,
        requestRole: OwnerIpcRequestRole,
    ): OwnerResponse {
        bundle.classLoader = AndroidHostReply::class.java.classLoader
        val protocolVersion = readProtocolVersion(bundle)
        val echoedClientSession = bundle.getString(MihomoControlService.CLIENT_SESSION_ID)
        val ownerSession = bundle.getString(MihomoControlService.OWNER_SESSION_ID)
        if (protocolVersion != AndroidHostProtocolVersion.V9 ||
            echoedClientSession != clientSessionId || ownerSession.isNullOrBlank()
        ) {
            return OwnerResponse.unavailable(OwnerClientTerminal.InvalidReply)
        }

        val reply = readReply(bundle)
            ?: return OwnerResponse.unavailable(OwnerClientTerminal.InvalidReply)
        if (!bundle.containsKey(MihomoControlService.STATUS_CODE)) {
            return OwnerResponse.unavailable(OwnerClientTerminal.InvalidReply)
        }
        val statusCode = bundle.getInt(MihomoControlService.STATUS_CODE)
        val payload = readOwnerResponsePayload(bundle)
            ?: return OwnerResponse.unavailable(OwnerClientTerminal.InvalidReply)
        val ownerSnapshotJson = when (payload) {
            is OwnerResponsePayload.VpnOwnerSnapshot -> payload.canonicalJson
            is OwnerResponsePayload.StopVpn -> payload.snapshotJson
            else -> null
        }
        val ownerSnapshot = ownerSnapshotJson?.let(VpnOwnerSnapshotWire::parse)
        if (requestRole == OwnerIpcRequestRole.VpnOwnerSnapshotReadback &&
            (payload !is OwnerResponsePayload.VpnOwnerSnapshot ||
                reply != AndroidHostReply.VPN_OWNER_SNAPSHOT_AVAILABLE ||
                statusCode != FfiErrorCode.OK.value.toInt() || ownerSnapshot == null ||
                ownerSnapshot.ownerSessionId != ownerSession)
        ) {
            return OwnerResponse.unavailable(OwnerClientTerminal.InvalidReply)
        }
        if (requestRole == OwnerIpcRequestRole.VpnOwnerSnapshotReadback &&
            ownerSnapshot?.core !is VpnOwnerCoreReadbackWire.Available
        ) {
            ownerSessionFence.requireReadback(
                requestId,
                expectedOwnerSession,
                OwnerClientTerminal.OwnerSessionReadbackRequired,
            )
            return OwnerResponse.unavailable(OwnerClientTerminal.OwnerSessionReadbackRequired)
        }
        if (requestRole == OwnerIpcRequestRole.VpnStop) {
            if (payload is OwnerResponsePayload.StopVpn &&
                (ownerSnapshot == null || ownerSnapshot.ownerSessionId != ownerSession)
            ) {
                return OwnerResponse.unavailable(OwnerClientTerminal.InvalidReply)
            }
            val stopReplyRequiresReadback = when (reply) {
                AndroidHostReply.VPN_STOPPED,
                AndroidHostReply.VPN_STOP_TIMED_OUT,
                AndroidHostReply.VPN_STOP_SUPERSEDED,
                AndroidHostReply.VPN_STOP_CANCELED,
                AndroidHostReply.FAILED -> true
                else -> false
            }
            if (stopReplyRequiresReadback &&
                payload !is OwnerResponsePayload.StopVpn
            ) {
                return OwnerResponse.unavailable(OwnerClientTerminal.InvalidReply)
            }
            if (reply == AndroidHostReply.VPN_STOPPED &&
                (statusCode != FfiErrorCode.OK.value.toInt() ||
                    ownerSnapshot?.isTerminalStopped() != true)
            ) {
                return OwnerResponse.unavailable(OwnerClientTerminal.InvalidReply)
            }
        }
        val coreSnapshot = (payload.coreReadback() as? OwnerCoreReadback.Available)?.snapshot
        val replyCarriesReadback = when (reply) {
            AndroidHostReply.CORE_LIFECYCLE_AVAILABLE ->
                requestRole == OwnerIpcRequestRole.SessionReadback
            AndroidHostReply.OWNER_SESSION_CHANGED -> true
            else -> false
        }
        val hasLifecycleReadback = coreSnapshot != null &&
            statusCode == FfiErrorCode.OK.value.toInt() &&
            replyCarriesReadback
        when (requestRole) {
            OwnerIpcRequestRole.VpnStop,
            OwnerIpcRequestRole.VpnOwnerSnapshotReadback -> Unit
            OwnerIpcRequestRole.Operation,
            OwnerIpcRequestRole.SessionReadback -> Unit
        }
        val evidence = when {
            ownerSnapshot?.core is VpnOwnerCoreReadbackWire.Available ->
                OwnerSessionEvidence.VpnOwnerSnapshotReadback
            ownerSnapshot != null && requestRole == OwnerIpcRequestRole.VpnStop ->
                OwnerSessionEvidence.VpnStopReadback
            hasLifecycleReadback -> OwnerSessionEvidence.CoreLifecycleReadback
            else -> OwnerSessionEvidence.OperationReply
        }
        val sessionAcceptance = ownerSessionFence.accept(
            expectedSessionId = expectedOwnerSession,
            responseSessionId = ownerSession,
            requestId = requestId,
            evidence = evidence,
        )
        when (sessionAcceptance) {
            OwnerSessionAcceptance.StaleResponse ->
                return OwnerResponse.staleOwnerSession(ownerSessionFence.current() ?: ownerSession)
            OwnerSessionAcceptance.ReadbackRequired -> return OwnerResponse.unavailable(
                when (reply) {
                    AndroidHostReply.SERVICE_UNAVAILABLE -> OwnerClientTerminal.OwnerUnavailable
                    else -> OwnerClientTerminal.OwnerSessionReadbackRequired
                },
            )
            OwnerSessionAcceptance.FirstObserved,
            OwnerSessionAcceptance.Current,
            OwnerSessionAcceptance.Changed -> Unit
        }
        if (sessionAcceptance == OwnerSessionAcceptance.Changed) {
            coreSnapshotFence.reset()
            vpnSnapshotFence.reset()
            ownerDataFence.reset()
        }
        if (ownerSnapshot != null &&
            vpnSnapshotFence.admit(VpnOwnerSnapshotVersion(ownerSession, requestId, ownerSnapshot)) ==
            VpnSnapshotAcceptance.Stale
        ) {
            ownerSessionFence.requireReadback(
                requestId,
                expectedOwnerSession,
                OwnerClientTerminal.VpnSnapshotStale,
            )
            return OwnerResponse.staleVpnSnapshot(ownerSession)
        }
        val serverReportedOwnerChange = reply == AndroidHostReply.OWNER_SESSION_CHANGED
        val ownerSessionChanged = sessionAcceptance == OwnerSessionAcceptance.Changed ||
            serverReportedOwnerChange
        val ownerCore = (ownerSnapshot?.core as? VpnOwnerCoreReadbackWire.Available)?.snapshot
        if (coreSnapshot != null) {
            val snapshotVersion = CoreSnapshotVersion(
                generation = coreSnapshot.generation,
                revision = coreSnapshot.revision,
                sessionToken = coreSnapshot.sessionToken,
                identity = coreSnapshot.identity,
                lifecycle = coreSnapshot.lifecycle,
                failure = coreSnapshot.failure,
            )
            when (coreSnapshotFence.admit(snapshotVersion)) {
                CoreSnapshotAcceptance.Accepted -> Unit
                CoreSnapshotAcceptance.Stale -> {
                    ownerSessionFence.requireReadback(
                        requestId,
                        expectedOwnerSession,
                        OwnerClientTerminal.CoreSnapshotStale,
                    )
                    return OwnerResponse.staleCoreSnapshot(ownerSession)
                }
            }
        }
        when (val ownerData = payload.ownerDataReadback()) {
            OwnerDataReadback.Unavailable -> Unit
            is OwnerDataReadback.Available -> {
                val (dataDomain, snapshotJson) = when (val facts = ownerData.facts) {
                    OwnerOperationDataFacts.RevisionOnly -> null to null
                    is OwnerOperationDataFacts.Domain -> facts.domain to null
                    is OwnerOperationDataFacts.Snapshot -> facts.domain to facts.json
                }
                if (ownerDataFence.admit(
                        ownerSessionId = ownerSession,
                        coreGeneration = ownerData.generation,
                        revision = ownerData.revision,
                        domain = dataDomain,
                        snapshotJson = snapshotJson,
                    ) == OwnerDataAcceptance.Stale
                ) {
                    ownerSessionFence.requireReadback(
                        requestId,
                        expectedOwnerSession,
                        OwnerClientTerminal.OwnerDataStale,
                    )
                    return OwnerResponse.staleOwnerData(ownerSession)
                }
            }
        }
        if (ownerCore != null) {
            val snapshotVersion = CoreSnapshotVersion(
                generation = ownerCore.generation,
                revision = ownerCore.revision,
                sessionToken = ownerCore.sessionToken,
                identity = ownerCore.identity,
                lifecycle = ownerCore.lifecycle,
                failure = ownerCore.failure,
            )
            if (coreSnapshotFence.admit(snapshotVersion) == CoreSnapshotAcceptance.Stale) {
                ownerSessionFence.requireReadback(
                    requestId,
                    expectedOwnerSession,
                    OwnerClientTerminal.CoreSnapshotStale,
                )
                return OwnerResponse.staleCoreSnapshot(ownerSession)
            }
        }
        return OwnerResponse(
            reply = if (ownerSessionChanged && requestRole != OwnerIpcRequestRole.VpnStop &&
                requestRole != OwnerIpcRequestRole.VpnOwnerSnapshotReadback
            ) {
                AndroidHostReply.OWNER_SESSION_CHANGED
            } else {
                reply
            },
            ownerSessionId = ownerSession,
            statusCode = statusCode,
            statusMessage = bundle.getString(MihomoControlService.STATUS_MESSAGE),
            payload = payload,
            terminal = when (reply) {
                AndroidHostReply.SERVICE_UNAVAILABLE -> OwnerClientTerminal.OwnerUnavailable
                else -> OwnerClientTerminal.ReplyReceived
            },
        )
    }

    private fun readOwnerResponsePayload(bundle: Bundle): OwnerResponsePayload? {
        return when (readEnum<OwnerResponsePayloadKind>(bundle, MihomoControlService.RESPONSE_PAYLOAD_KIND)) {
        OwnerResponsePayloadKind.Failure -> OwnerResponsePayload.Failure
        OwnerResponsePayloadKind.CoreLifecycleSnapshot ->
            readCoreSnapshot(bundle)?.let { OwnerResponsePayload.CoreLifecycleSnapshot(it) }
        OwnerResponsePayloadKind.VpnOwnerSnapshot -> bundle.getString(
            MihomoControlService.VPN_OWNER_SNAPSHOT_JSON,
        )?.let { OwnerResponsePayload.VpnOwnerSnapshot(it) }
        OwnerResponsePayloadKind.SurfaceSnapshot -> {
            val json = bundle.getString(MihomoControlService.SURFACE_SNAPSHOT_JSON) ?: return null
            val core = readCoreReadback(bundle) ?: return null
            OwnerResponsePayload.SurfaceSnapshot(json, core)
        }
        OwnerResponsePayloadKind.OwnerDataRevision -> {
            if (!bundle.containsKey(MihomoControlService.OWNER_DATA_REVISION) ||
                !bundle.containsKey(MihomoControlService.OWNER_DATA_GENERATION)
            ) return null
            val core = readCoreReadback(bundle) ?: return null
            val revision = bundle.getLong(MihomoControlService.OWNER_DATA_REVISION)
            val generation = bundle.getLong(MihomoControlService.OWNER_DATA_GENERATION)
            if (revision < 0L || generation < 0L) return null
            OwnerResponsePayload.OwnerDataRevision(
                revision = revision,
                generation = generation,
                coreReadback = core,
            )
        }
        OwnerResponsePayloadKind.Authorization -> readEnum<OwnerVpnAuthorization>(
            bundle,
            MihomoControlService.VPN_AUTHORIZATION_STATE,
        )?.let { OwnerResponsePayload.Authorization(it) }
        OwnerResponsePayloadKind.ProductCommand -> {
            val json = bundle.getString(MihomoControlService.PRODUCT_COMMAND_RESULT_JSON) ?: return null
            val core = readCoreReadback(bundle) ?: return null
            OwnerResponsePayload.ProductCommand(json, core)
        }
        OwnerResponsePayloadKind.OwnerOperation -> {
            val json = bundle.getString(MihomoControlService.OWNER_OPERATION_RESULT_JSON) ?: return null
            if (!bundle.containsKey(MihomoControlService.OWNER_DATA_REVISION) ||
                !bundle.containsKey(MihomoControlService.OWNER_DATA_GENERATION)
            ) return null
            val factsKind = readEnum<OwnerOperationDataFactsKind>(
                bundle,
                MihomoControlService.OWNER_OPERATION_DATA_FACTS_KIND,
            ) ?: return null
            val facts = when (factsKind) {
                OwnerOperationDataFactsKind.RevisionOnly -> OwnerOperationDataFacts.RevisionOnly
                OwnerOperationDataFactsKind.Domain -> {
                    val domain = readEnum<AndroidOwnerDataDomain>(
                        bundle,
                        MihomoControlService.OWNER_OPERATION_DATA_DOMAIN,
                    ) ?: return null
                    OwnerOperationDataFacts.Domain(domain)
                }
                OwnerOperationDataFactsKind.Snapshot -> {
                    val domain = readEnum<AndroidOwnerDataDomain>(
                        bundle,
                        MihomoControlService.OWNER_OPERATION_DATA_DOMAIN,
                    ) ?: return null
                    val snapshot = bundle.getString(
                        MihomoControlService.OWNER_OPERATION_DATA_SNAPSHOT_JSON,
                    ) ?: return null
                    OwnerOperationDataFacts.Snapshot(domain, snapshot)
                }
            }
            val core = readCoreReadback(bundle) ?: return null
            val revision = bundle.getLong(MihomoControlService.OWNER_DATA_REVISION)
            val generation = bundle.getLong(MihomoControlService.OWNER_DATA_GENERATION)
            if (revision < 0L || generation < 0L) return null
            OwnerResponsePayload.OwnerOperation(
                outcomeJson = json,
                revision = revision,
                generation = generation,
                dataFacts = facts,
                coreReadback = core,
            )
        }
        OwnerResponsePayloadKind.StopVpn -> bundle.getString(
            MihomoControlService.VPN_STOP_SNAPSHOT_JSON,
        )?.let { OwnerResponsePayload.StopVpn(it) }
        OwnerResponsePayloadKind.VpnStart -> readEnum<VpnStartOutcomeWire>(
            bundle,
            MihomoControlService.VPN_START_OUTCOME,
        )?.let { OwnerResponsePayload.VpnStart(it) }
        OwnerResponsePayloadKind.VpnConfiguration -> readEnum<VpnConfigurationApplyOutcomeWire>(
            bundle,
            MihomoControlService.VPN_CONFIGURATION_OUTCOME,
        )?.let { OwnerResponsePayload.VpnConfiguration(it) }
        OwnerResponsePayloadKind.OwnerSessionReadback -> {
            if (!bundle.containsKey(MihomoControlService.OWNER_DATA_REVISION)) return null
            val core = readCoreReadback(bundle) ?: return null
            val revision = bundle.getLong(MihomoControlService.OWNER_DATA_REVISION)
            if (revision < 0L) return null
            OwnerResponsePayload.OwnerSessionReadback(
                ownerDataRevision = revision,
                coreReadback = core,
            )
        }
        null -> null
        }
    }

    private fun readCoreReadback(bundle: Bundle): OwnerCoreReadback? = when (
        readEnum<OwnerCoreReadbackKind>(bundle, MihomoControlService.CORE_READBACK_KIND)
    ) {
        OwnerCoreReadbackKind.Unavailable -> OwnerCoreReadback.Unavailable
        OwnerCoreReadbackKind.Available -> readCoreSnapshot(bundle)?.let { OwnerCoreReadback.Available(it) }
        null -> null
    }

    private fun readCoreSnapshot(bundle: Bundle): OwnerCoreSnapshot? {
        val requiredKeys = listOf(
            MihomoControlService.CORE_SNAPSHOT_LIFECYCLE,
            MihomoControlService.CORE_SNAPSHOT_GENERATION,
            MihomoControlService.CORE_SNAPSHOT_SESSION_TOKEN,
            MihomoControlService.CORE_SNAPSHOT_REVISION,
            MihomoControlService.CORE_SNAPSHOT_IDENTITY_JSON,
            MihomoControlService.CORE_SNAPSHOT_FAILURE_JSON,
        )
        if (requiredKeys.any { !bundle.containsKey(it) }) return null
        val lifecycle = readEnum<CoreOwnerLifecycleWire>(
            bundle,
            MihomoControlService.CORE_SNAPSHOT_LIFECYCLE,
        ) ?: return null
        val generation = bundle.getLong(MihomoControlService.CORE_SNAPSHOT_GENERATION)
        val revision = bundle.getLong(MihomoControlService.CORE_SNAPSHOT_REVISION)
        if (generation < 0L || revision < 0L) return null
        val identity = bundle.getString(MihomoControlService.CORE_SNAPSHOT_IDENTITY_JSON)
            ?.let(CoreOwnerIdentityWire::parse) ?: return null
        val failure = when (val rawFailure = bundle.getString(MihomoControlService.CORE_SNAPSHOT_FAILURE_JSON)) {
            null -> null
            else -> CoreOwnerFailureWire.parse(rawFailure) ?: return null
        }
        return OwnerCoreSnapshot(
            lifecycle = lifecycle,
            generation = generation,
            sessionToken = bundle.getString(MihomoControlService.CORE_SNAPSHOT_SESSION_TOKEN),
            revision = revision,
            identity = identity,
            failure = failure,
        )
    }

    private inline fun <reified T : Enum<T>> readEnum(bundle: Bundle, key: String): T? {
        bundle.classLoader = T::class.java.classLoader
        return if (android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.TIRAMISU) {
            bundle.getSerializable(key, T::class.java)
        } else {
            @Suppress("DEPRECATION")
            bundle.getSerializable(key) as? T
        }
    }

    private fun sendCancellation(
        request: infiltrator_android.AndroidHostRequest,
        requestId: Long,
        messenger: Messenger,
    ) {
        val cancelCode = if (request == infiltrator_android.AndroidHostRequest.START_VPN_SERVICE) {
            MihomoControlService.CANCEL_START_REQUEST
        } else {
            MihomoControlService.CANCEL_OWNER_REQUEST
        }
        val cancellation = Message.obtain(null, cancelCode).apply {
            data = Bundle().apply {
                putSerializable(
                    MihomoControlService.PROTOCOL_VERSION,
                    AndroidHostProtocolVersion.V9,
                )
                putString(MihomoControlService.CLIENT_SESSION_ID, clientSessionId)
                putLong(MihomoControlService.REQUEST_ID, requestId)
            }
        }
        try {
            messenger.send(cancellation)
        } catch (error: RemoteException) {
            Log.w(TAG, "owner IPC cancellation send failed: ${error.message}")
        }
    }

    private fun newRequestDeadline(): VpnOwnerRequestDeadline = VpnOwnerRequestDeadline.after(
        SystemClock.elapsedRealtime(),
        REQUEST_TIMEOUT_MILLIS,
    )

    private fun acknowledgeOwnerStartRequest(requestId: Long, messenger: Messenger) {
        val acknowledgment = Message.obtain(null, MihomoControlService.ACK_START_REQUEST).apply {
            data = Bundle().apply {
                putSerializable(
                    MihomoControlService.PROTOCOL_VERSION,
                    AndroidHostProtocolVersion.V9,
                )
                putString(MihomoControlService.CLIENT_SESSION_ID, clientSessionId)
                putLong(MihomoControlService.REQUEST_ID, requestId)
            }
        }
        try {
            messenger.send(acknowledgment)
        } catch (error: RemoteException) {
            Log.w(TAG, "VPN start acknowledgment failed: ${error.message}")
        }
    }

    private fun readReply(bundle: Bundle): AndroidHostReply? =
        if (android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.TIRAMISU) {
            bundle.getSerializable(MihomoControlService.RESPONSE_VALUE, AndroidHostReply::class.java)
        } else {
            @Suppress("DEPRECATION")
            bundle.getSerializable(MihomoControlService.RESPONSE_VALUE) as? AndroidHostReply
        }

    private fun readProtocolVersion(bundle: Bundle): AndroidHostProtocolVersion? {
        bundle.classLoader = AndroidHostProtocolVersion::class.java.classLoader
        return if (android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.TIRAMISU) {
            bundle.getSerializable(
                MihomoControlService.PROTOCOL_VERSION,
                AndroidHostProtocolVersion::class.java,
            )
        } else {
            @Suppress("DEPRECATION")
            bundle.getSerializable(MihomoControlService.PROTOCOL_VERSION)
                as? AndroidHostProtocolVersion
        }
    }

    private data class OwnerResponse(
        val reply: AndroidHostReply,
        val ownerSessionId: String?,
        val statusCode: Int,
        val statusMessage: String?,
        val payload: OwnerResponsePayload = OwnerResponsePayload.Failure,
        val terminal: OwnerClientTerminal = OwnerClientTerminal.ReplyReceived,
    ) {
        companion object {
            fun unavailable(
                terminal: OwnerClientTerminal = OwnerClientTerminal.OwnerUnavailable,
            ) = OwnerResponse(
                reply = AndroidHostReply.SERVICE_UNAVAILABLE,
                ownerSessionId = null,
                statusCode = FfiErrorCode.NOT_READY.value.toInt(),
                statusMessage = "Android :vpn owner is unavailable",
                terminal = terminal,
            )

            fun staleOwnerSession(sessionId: String) = OwnerResponse(
                reply = AndroidHostReply.STALE_SNAPSHOT,
                ownerSessionId = sessionId,
                statusCode = FfiErrorCode.NOT_READY.value.toInt(),
                statusMessage = "stale Android owner session reply was rejected",
                terminal = OwnerClientTerminal.OwnerSessionStale,
            )

            fun staleCoreSnapshot(sessionId: String) = OwnerResponse(
                reply = AndroidHostReply.STALE_SNAPSHOT,
                ownerSessionId = sessionId,
                statusCode = FfiErrorCode.NOT_READY.value.toInt(),
                statusMessage = "stale Core snapshot was rejected",
                terminal = OwnerClientTerminal.CoreSnapshotStale,
            )

            fun staleVpnSnapshot(sessionId: String) = OwnerResponse(
                reply = AndroidHostReply.STALE_SNAPSHOT,
                ownerSessionId = sessionId,
                statusCode = FfiErrorCode.NOT_READY.value.toInt(),
                statusMessage = "stale VPN generation readback was rejected",
                terminal = OwnerClientTerminal.VpnSnapshotStale,
            )

            fun staleOwnerData(sessionId: String) = OwnerResponse(
                reply = AndroidHostReply.STALE_OWNER_DATA,
                ownerSessionId = sessionId,
                statusCode = FfiErrorCode.NOT_READY.value.toInt(),
                statusMessage = "stale Android owner data was rejected",
                terminal = OwnerClientTerminal.OwnerDataStale,
            )
        }
    }

    private companion object {
        private const val TAG = "MihomoControlClient"
        private const val REQUEST_TIMEOUT_MILLIS = 15_000L
        private val REQUEST_IDS = AtomicLong(1)
    }
}

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

/**
 * Process owner for the Mihomo child process and the VPN service.
 * Starting/binding this service only makes the host available; it does not
 * start Mihomo or establish a VPN interface.
 */
class MihomoControlService : Service() {
    private enum class OwnerServicePhase {
        Initializing,
        Accepting,
        Closing,
        Unavailable,
    }

    private lateinit var commandThread: HandlerThread
    private lateinit var endpoint: Messenger
    internal lateinit var commandHandler: Handler
    internal lateinit var ownerSessionId: String
    @Volatile
    private var ownerServicePhase: OwnerServicePhase = OwnerServicePhase.Initializing
    private var ownerSessionStartJob: Job? = null
    internal lateinit var vpnStartExecutor: ThreadPoolExecutor
    internal val vpnStartAdmission = VpnOwnerStartRequestAdmission()
    internal val vpnStartTasks = ConcurrentHashMap<VpnOwnerRequestKey, FutureTask<Unit>>()
    internal val vpnStartTerminalCache = VpnOwnerStartTerminalCache<ControlServiceResponse>()
    internal val vpnStartRepliesLock = Any()
    internal val vpnStartReplies = mutableMapOf<VpnOwnerRequestKey, MutableList<Message>>()
    private val ownerRequestAdmission = OwnerIpcRequestAdmission<ControlServiceResponse>()
    private val ownerRequestLock = Any()
    private val ownerRequestReplies = mutableMapOf<OwnerIpcRequestKey, MutableList<Message>>()
    private val ownerRequestJobs = mutableMapOf<OwnerIpcRequestKey, Job>()
    internal val ownerRequestSensitivePayloads = OwnerIpcSensitivePayloadStore()

    internal suspend fun canonicalSurfaceSnapshotRouteCheck(): Any =
        when (AndroidHostRequest.READ_SURFACE_SNAPSHOT) {
            AndroidHostRequest.READ_SURFACE_SNAPSHOT -> androidSurfaceSnapshotJson()
            else -> Unit
        }

    override fun onCreate() {
        super.onCreate()
        ownerSessionId = UUID.randomUUID().toString()
        val ownerApplication = application as? MihomoApplication
        ownerServicePhase = OwnerServicePhase.Initializing
        ownerSessionStartJob = ownerApplication?.let { app ->
            app.prepareVpnOwnerSession(ownerSessionId)
            app.ownerIpcScope.launch {
                val status = androidBeginCoreOwnerSession()
                if (ownerServicePhase != OwnerServicePhase.Initializing) return@launch
                if (status.code != FfiErrorCode.OK) {
                    ownerServicePhase = OwnerServicePhase.Unavailable
                    val reason = status.message ?: "Core owner session initialization failed"
                    app.failVpnOwnerSession(ownerSessionId, reason)
                    Log.e(TAG, reason)
                    return@launch
                }
                when (app.activateVpnOwnerSession(ownerSessionId)) {
                    VpnOwnerSessionActivationOutcome.Activated,
                    VpnOwnerSessionActivationOutcome.AlreadyActive ->
                        ownerServicePhase = OwnerServicePhase.Accepting
                    VpnOwnerSessionActivationOutcome.StaleSession -> {
                        ownerServicePhase = OwnerServicePhase.Unavailable
                        val reason = "VPN owner snapshot session changed during initialization"
                        app.failVpnOwnerSession(ownerSessionId, reason)
                        Log.e(TAG, reason)
                    }
                }
            }
        } ?: run {
            ownerServicePhase = OwnerServicePhase.Unavailable
            Log.e(TAG, "MihomoApplication is unavailable for Core owner initialization")
            null
        }
        commandThread = HandlerThread("mihomo-control-ipc").also { it.start() }
        commandHandler = Handler(commandThread.looper, Handler.Callback(::handleMessage))
        endpoint = Messenger(commandHandler)
        vpnStartExecutor = ThreadPoolExecutor(
            1,
            1,
            0L,
            TimeUnit.MILLISECONDS,
            ArrayBlockingQueue(1),
        ) { runnable -> Thread(runnable, "mihomo-vpn-start-owner") }
        Log.i(TAG, "control host ready; core remains stopped until a typed request")
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int = START_STICKY

    override fun onBind(intent: Intent?): IBinder = endpoint.binder

    override fun onDestroy() {
        ownerServicePhase = OwnerServicePhase.Closing
        for (key in vpnStartAdmission.activeKeys()) {
            when (val cancellation = vpnStartAdmission.cancel(key)) {
                is VpnOwnerStartCancellation.CancelGeneration -> {
                    clearVpnStartReplies(key)
                    cacheCanceledVpnStart(key)
                    MihomoVpnService.cancelUndispatchedStart(cancellation.generation)
                }
                VpnOwnerStartCancellation.ExecutionAlreadyStarted,
                VpnOwnerStartCancellation.TerminalReadback -> Unit
                VpnOwnerStartCancellation.CanceledBeforeGeneration -> {
                    clearVpnStartReplies(key)
                    cacheCanceledVpnStart(key)
                    val task = vpnStartTasks.remove(key)
                    if (task != null && task.cancel(false)) {
                        vpnStartExecutor.remove(task)
                        vpnStartAdmission.complete(key)
                    }
                }
                VpnOwnerStartCancellation.AlreadyCanceled,
                VpnOwnerStartCancellation.Unknown -> Unit
            }
        }
        for (key in ownerRequestAdmission.activeKeys()) {
            when (ownerRequestAdmission.cancel(key)) {
                OwnerIpcCancellation.CanceledBeforeExecution -> {
                    val (job, waiters) = synchronized(ownerRequestLock) {
                        ownerRequestJobs.remove(key) to ownerRequestReplies.remove(key).orEmpty().toList()
                    }
                    wipeOwnerRequestSensitivePayload(key)
                    job?.cancel()
                    waiters.forEach { requestMessage ->
                        sendResponse(
                            requestMessage,
                            key.requestId,
                            key.clientSessionId,
                            ControlServiceResponse(
                                AndroidHostReply.SERVICE_UNAVAILABLE,
                                FfiErrorCode.NOT_READY,
                                "Android owner service is shutting down",
                            ),
                        )
                    }
                }
                OwnerIpcCancellation.AlreadyCanceled,
                OwnerIpcCancellation.ExecutionAlreadyStarted,
                is OwnerIpcCancellation.TerminalReadback,
                OwnerIpcCancellation.Unknown -> Unit
            }
        }
        vpnStartExecutor.shutdownNow()
        commandThread.quitSafely()
        val ownerApplication = application as? MihomoApplication
        val startup = ownerSessionStartJob
        ownerApplication?.let { app ->
            app.ownerIpcScope.launch {
                startup?.join()
                val status = androidCloseCoreOwner()
                if (status.code != FfiErrorCode.OK) {
                    Log.e(TAG, "Core owner terminal close failed: ${status.message}")
                }
                app.endVpnOwnerSession(ownerSessionId)
            }
        }
        super.onDestroy()
    }

    private fun handleMessage(message: Message): Boolean {
        if (message.what == CANCEL_START_REQUEST) {
            cancelStartRequest(message)
            return true
        }
        if (message.what == CANCEL_OWNER_REQUEST) {
            cancelOwnerRequest(message)
            return true
        }
        if (message.what == ACK_START_REQUEST) {
            acknowledgeStartRequest(message)
            return true
        }
        if (message.what != REQUEST) return false
        val arguments = runCatching { Bundle(message.data) }.getOrNull()
        if (arguments == null) {
            runCatching { wipeSensitiveBundle(message.data) }
            sendResponse(
                message,
                Long.MIN_VALUE,
                null,
                ControlServiceResponse(AndroidHostReply.FAILED, FfiErrorCode.INVALID_INPUT, "request bundle unavailable"),
            )
            return true
        }
        val requestId = runCatching { arguments.getLong(REQUEST_ID, Long.MIN_VALUE) }
            .getOrDefault(Long.MIN_VALUE)
        val clientSessionId = runCatching { arguments.getString(CLIENT_SESSION_ID) }
            .getOrNull()
        if (ownerServicePhase != OwnerServicePhase.Accepting) {
            wipeSensitiveRequest(message, arguments)
            sendResponse(
                message,
                requestId,
                clientSessionId,
                ControlServiceResponse(
                    AndroidHostReply.SERVICE_UNAVAILABLE,
                    FfiErrorCode.NOT_READY,
                    "Android Core owner session is not accepting requests",
                ),
            )
            return true
        }
        val protocolVersion = runCatching { readProtocolVersion(arguments) }.getOrNull()
        val expectedOwnerSession = runCatching { arguments.getString(EXPECTED_OWNER_SESSION_ID) }
            .getOrNull()
        val request = runCatching { readRequest(arguments) }.getOrNull()
        val result = when {
            protocolVersion != AndroidHostProtocolVersion.V10 || clientSessionId.isNullOrBlank() || requestId <= 0L -> {
                wipeSensitiveRequest(message, arguments)
                ControlServiceResponse(AndroidHostReply.SERVICE_UNAVAILABLE, FfiErrorCode.NOT_READY)
            }
            request == null -> {
                wipeSensitiveRequest(message, arguments)
                ControlServiceResponse(AndroidHostReply.FAILED, FfiErrorCode.INVALID_INPUT)
            }
            request == AndroidHostRequest.START_VPN_SERVICE -> {
                val deadline = runCatching { readRequestDeadline(arguments) }.getOrNull()
                if (deadline == null) {
                    wipeSensitiveRequest(message, arguments)
                    ControlServiceResponse(
                        AndroidHostReply.VPN_START_TIMED_OUT,
                        FfiErrorCode.NOT_READY,
                        "VPN start request omitted its absolute deadline",
                    )
                } else {
                    wipeSensitiveRequest(message, arguments)
                    enqueueVpnStart(message, requireNotNull(clientSessionId), requestId, deadline)
                    return true
                }
            }
            else -> {
                enqueueOwnerRequest(
                    message = message,
                    arguments = arguments,
                    request = requireNotNull(request),
                    clientSessionId = requireNotNull(clientSessionId),
                    requestId = requestId,
                    expectedOwnerSession = expectedOwnerSession,
                )
                return true
            }
        }
        sendResponse(message, requestId, clientSessionId, result)
        return true
    }

    private fun enqueueVpnStart(
        message: Message,
        clientSessionId: String,
        requestId: Long,
        deadline: VpnOwnerRequestDeadline,
    ) {
        val key = VpnOwnerRequestKey(clientSessionId, requestId)
        val now = SystemClock.elapsedRealtime()
        when (val replay = vpnStartTerminalCache.read(key, now)) {
            is VpnOwnerStartReplayLookup.Hit -> {
                sendResponse(message, requestId, clientSessionId, replay.response)
                return
            }
            VpnOwnerStartReplayLookup.Miss -> Unit
        }
        fun sendTerminal(response: ControlServiceResponse) {
            vpnStartTerminalCache.store(key, response, SystemClock.elapsedRealtime())
            sendResponse(message, requestId, clientSessionId, response)
        }
        val admission = vpnStartAdmission.admit(key, deadline, now)
        when (admission) {
            VpnOwnerStartAdmission.Expired -> sendTerminal(
                ControlServiceResponse(
                    AndroidHostReply.VPN_START_TIMED_OUT,
                    FfiErrorCode.NOT_READY,
                    "VPN start request expired before owner admission",
                    OwnerResponsePayload.VpnStart(VpnStartOutcomeWire.TimedOut),
                ),
            )
            VpnOwnerStartAdmission.Duplicate -> when (attachVpnStartReply(key, message)) {
                VpnStartReplyAdmission.Attached -> Unit
                VpnStartReplyAdmission.CapacityExceeded -> sendResponse(
                    message,
                    requestId,
                    clientSessionId,
                    ControlServiceResponse(
                        AndroidHostReply.SERVICE_UNAVAILABLE,
                        FfiErrorCode.NOT_READY,
                        "duplicate VPN start waiter limit reached",
                    ),
                )
                VpnStartReplyAdmission.NoActiveRequest -> when (
                    val replay = vpnStartTerminalCache.read(key, SystemClock.elapsedRealtime())
                ) {
                    is VpnOwnerStartReplayLookup.Hit ->
                        sendResponse(message, requestId, clientSessionId, replay.response)
                    VpnOwnerStartReplayLookup.Miss -> sendTerminal(
                        ControlServiceResponse(
                            AndroidHostReply.FAILED,
                            FfiErrorCode.INVALID_STATE,
                            "VPN start request no longer has an active terminal result",
                        ),
                    )
                }
            }
            VpnOwnerStartAdmission.QueueFull -> sendTerminal(
                ControlServiceResponse(
                    AndroidHostReply.SERVICE_UNAVAILABLE,
                    FfiErrorCode.NOT_READY,
                    "VPN start owner queue is full",
                ),
            )
            VpnOwnerStartAdmission.Admitted -> {
                synchronized(vpnStartRepliesLock) {
                    vpnStartReplies[key] = mutableListOf(message)
                }
                val task = FutureTask {
                    executeVpnStart(key, deadline)
                    Unit
                }
                vpnStartTasks[key] = task
                try {
                    vpnStartExecutor.execute(task)
                } catch (_: RejectedExecutionException) {
                    vpnStartTasks.remove(key)
                    val rejected = ControlServiceResponse(
                            AndroidHostReply.SERVICE_UNAVAILABLE,
                            FfiErrorCode.NOT_READY,
                            "VPN start owner queue rejected the request",
                            OwnerResponsePayload.VpnStart(VpnStartOutcomeWire.OwnerUnavailable),
                        )
                    vpnStartTerminalCache.store(key, rejected, SystemClock.elapsedRealtime())
                    vpnStartAdmission.complete(key)
                    scheduleStartExpiry(key, deadline)
                    publishVpnStartReplies(key, rejected)
                }
            }
        }
    }

    private fun enqueueOwnerRequest(
        message: Message,
        arguments: Bundle,
        request: AndroidHostRequest,
        clientSessionId: String,
        requestId: Long,
        expectedOwnerSession: String?,
    ) {
        val deadline = runCatching { readRequestDeadline(arguments) }.getOrNull()
        if (deadline == null) {
            wipeSensitiveRequest(message, arguments)
            sendResponse(
                message,
                requestId,
                clientSessionId,
                ControlServiceResponse(
                    AndroidHostReply.SERVICE_UNAVAILABLE,
                    FfiErrorCode.NOT_READY,
                    "owner request omitted its absolute deadline",
                ),
            )
            return
        }
        val key = OwnerIpcRequestKey(clientSessionId, requestId)
        val fingerprint = runCatching { requestFingerprint(request, arguments) }.getOrElse {
            wipeSensitiveRequest(message, arguments)
            sendResponse(
                message,
                requestId,
                clientSessionId,
                ControlServiceResponse(
                    AndroidHostReply.FAILED,
                    FfiErrorCode.INVALID_INPUT,
                    "owner request payload is malformed",
                ),
            )
            return
        }
        val admission = ownerRequestAdmission.admit(
            key = key,
            fingerprint = fingerprint,
            deadline = deadline,
            nowElapsedRealtimeMillis = SystemClock.elapsedRealtime(),
        )
        if (admission != OwnerIpcAdmission.Admitted) wipeSensitiveRequest(message, arguments)
        when (admission) {
            OwnerIpcAdmission.Admitted -> {
                retainOwnerRequestSensitivePayload(key, request, message, arguments)
                synchronized(ownerRequestLock) {
                    ownerRequestReplies[key] = mutableListOf(message)
                }
                val job = (application as MihomoApplication).ownerIpcScope.launch(
                    start = CoroutineStart.LAZY,
                ) {
                    val execution = ownerRequestAdmission.begin(key, SystemClock.elapsedRealtime())
                    val response = when (execution) {
                        OwnerIpcExecutionStart.Started -> executeAdmitted(
                            key,
                            request,
                            arguments,
                            expectedOwnerSession,
                        )
                        OwnerIpcExecutionStart.Canceled -> ControlServiceResponse(
                            AndroidHostReply.FAILED,
                            FfiErrorCode.NOT_READY,
                            "owner request was canceled before execution",
                        )
                        OwnerIpcExecutionStart.Expired -> ControlServiceResponse(
                            AndroidHostReply.SERVICE_UNAVAILABLE,
                            FfiErrorCode.NOT_READY,
                            "owner request expired before execution",
                        )
                        OwnerIpcExecutionStart.InFlight -> ControlServiceResponse(
                            AndroidHostReply.FAILED,
                            FfiErrorCode.INVALID_STATE,
                            "owner request is already executing",
                        )
                        is OwnerIpcExecutionStart.TerminalReadback -> execution.response
                        OwnerIpcExecutionStart.Unknown -> ControlServiceResponse(
                            AndroidHostReply.FAILED,
                            FfiErrorCode.INVALID_STATE,
                            "owner request admission is no longer active",
                        )
                    }
                    when (
                        ownerRequestAdmission.complete(
                            key,
                            response,
                            SystemClock.elapsedRealtime(),
                        )
                    ) {
                        OwnerIpcCompletion.TerminalCached -> publishOwnerRequest(key, response)
                        OwnerIpcCompletion.SuppressCanceledCompletion,
                        OwnerIpcCompletion.Unknown -> Log.i(
                            TAG,
                            "suppressed late owner IPC receipt request=${key.requestId}",
                        )
                    }
                    if (execution is OwnerIpcExecutionStart.TerminalReadback) {
                        publishOwnerRequest(key, execution.response)
                    }
                }
                job.invokeOnCompletion { wipeOwnerRequestSensitivePayload(key) }
                synchronized(ownerRequestLock) {
                    ownerRequestJobs[key] = job
                }
                job.start()
            }
            OwnerIpcAdmission.InFlight -> when (attachOwnerRequestWaiter(key, message)) {
                OwnerIpcWaiterAdmission.Attached -> Unit
                OwnerIpcWaiterAdmission.CapacityExceeded -> sendResponse(
                    message,
                    requestId,
                    clientSessionId,
                    ControlServiceResponse(
                        AndroidHostReply.SERVICE_UNAVAILABLE,
                        FfiErrorCode.NOT_READY,
                        "duplicate owner IPC waiter limit reached",
                    ),
                )
                OwnerIpcWaiterAdmission.NoActiveRequest -> sendResponse(
                    message,
                    requestId,
                    clientSessionId,
                    ControlServiceResponse(
                        AndroidHostReply.FAILED,
                        FfiErrorCode.INVALID_STATE,
                        "owner request no longer has an active terminal waiter",
                    ),
                )
            }
            OwnerIpcAdmission.CanceledBeforeExecution -> sendResponse(
                message,
                requestId,
                clientSessionId,
                ControlServiceResponse(
                    AndroidHostReply.FAILED,
                    FfiErrorCode.NOT_READY,
                    "owner request was canceled before execution",
                ),
            )
            is OwnerIpcAdmission.TerminalReadback -> sendResponse(
                message,
                requestId,
                clientSessionId,
                admission.response,
            )
            OwnerIpcAdmission.QueueFull -> sendResponse(
                message,
                requestId,
                clientSessionId,
                ControlServiceResponse(
                    AndroidHostReply.SERVICE_UNAVAILABLE,
                    FfiErrorCode.NOT_READY,
                    "owner IPC request queue is full",
                ),
            )
            OwnerIpcAdmission.Expired -> sendResponse(
                message,
                requestId,
                clientSessionId,
                ControlServiceResponse(
                    AndroidHostReply.SERVICE_UNAVAILABLE,
                    FfiErrorCode.NOT_READY,
                    "owner request expired before admission",
                ),
            )
            OwnerIpcAdmission.RequestIdConflict -> sendResponse(
                message,
                requestId,
                clientSessionId,
                ControlServiceResponse(
                    AndroidHostReply.FAILED,
                    FfiErrorCode.INVALID_STATE,
                    "owner request ID was reused with different input",
                ),
            )
        }
    }

    private fun publishOwnerRequest(key: OwnerIpcRequestKey, response: ControlServiceResponse) {
        val waiters = synchronized(ownerRequestLock) {
            ownerRequestJobs.remove(key)
            ownerRequestReplies.remove(key).orEmpty().toList()
        }
        waiters.forEach { requestMessage ->
            sendResponse(
                requestMessage,
                key.requestId,
                key.clientSessionId,
                response,
            )
        }
    }

    private fun attachOwnerRequestWaiter(
        key: OwnerIpcRequestKey,
        message: Message,
    ): OwnerIpcWaiterAdmission = synchronized(ownerRequestLock) {
        val waiters = ownerRequestReplies[key] ?: return@synchronized OwnerIpcWaiterAdmission.NoActiveRequest
        if (waiters.size >= MAX_OWNER_REQUEST_WAITERS) {
            OwnerIpcWaiterAdmission.CapacityExceeded
        } else {
            waiters += message
            OwnerIpcWaiterAdmission.Attached
        }
    }

    private fun requestFingerprint(request: AndroidHostRequest, arguments: Bundle): String {
        val values = arguments.keySet()
            .filterNot {
                it == REQUEST_DEADLINE_ELAPSED_REALTIME ||
                    it == PROFILE_AGE_CREDENTIAL_PRIVATE_KEY_BYTES
            }
            .sorted()
            .joinToString("\u0000") { key ->
                "$key=${arguments.get(key)}"
            }
        return "$request\u0000$values"
    }

    private fun wipeSensitiveBundle(arguments: Bundle) {
        runCatching {
            arguments.getByteArray(PROFILE_AGE_CREDENTIAL_PRIVATE_KEY_BYTES)
        }.getOrNull()?.fill(0)
        arguments.remove(PROFILE_AGE_CREDENTIAL_PRIVATE_KEY_BYTES)
    }

    private fun wipeSensitiveRequest(message: Message, arguments: Bundle) {
        wipeSensitiveBundle(arguments)
        runCatching { wipeSensitiveBundle(message.data) }
    }

    private fun retainOwnerRequestSensitivePayload(
        key: OwnerIpcRequestKey,
        request: AndroidHostRequest,
        message: Message,
        arguments: Bundle,
    ) {
        if (request == AndroidHostRequest.EXECUTE_PROFILE_AGE_CREDENTIAL_OWNER_OPERATION) {
            val payload = runCatching {
                arguments.getByteArray(PROFILE_AGE_CREDENTIAL_PRIVATE_KEY_BYTES)?.copyOf()
            }.getOrNull() ?: ByteArray(0)
            ownerRequestSensitivePayloads.retain(key, payload)
        }
        wipeSensitiveRequest(message, arguments)
    }

    internal fun wipeOwnerRequestSensitivePayload(key: OwnerIpcRequestKey) {
        when (ownerRequestSensitivePayloads.removeAndWipe(key)) {
            OwnerIpcSensitivePayloadRemoval.Wiped,
            OwnerIpcSensitivePayloadRemoval.Missing -> Unit
        }
    }

    private fun cancelOwnerRequest(message: Message) {
        val arguments = runCatching { message.data }.getOrNull() ?: return
        arguments.classLoader = AndroidHostProtocolVersion::class.java.classLoader
        val key = OwnerIpcRequestKey(
            clientSessionId = arguments.getString(CLIENT_SESSION_ID).orEmpty(),
            requestId = arguments.getLong(REQUEST_ID, Long.MIN_VALUE),
        )
        if (readProtocolVersion(arguments) != AndroidHostProtocolVersion.V10 ||
            key.clientSessionId.isBlank() || key.requestId <= 0L
        ) {
            Log.w(TAG, "ignored malformed owner IPC cancellation")
            return
        }
        when (ownerRequestAdmission.cancel(key)) {
            OwnerIpcCancellation.CanceledBeforeExecution -> {
                val (job, waiters) = synchronized(ownerRequestLock) {
                    ownerRequestJobs.remove(key) to ownerRequestReplies.remove(key).orEmpty().toList()
                }
                wipeOwnerRequestSensitivePayload(key)
                job?.cancel()
                waiters.forEach { requestMessage ->
                    sendResponse(
                        requestMessage,
                        key.requestId,
                        key.clientSessionId,
                        ControlServiceResponse(
                            AndroidHostReply.FAILED,
                            FfiErrorCode.NOT_READY,
                            "owner request was canceled before execution",
                        ),
                    )
                }
            }
            OwnerIpcCancellation.AlreadyCanceled,
            OwnerIpcCancellation.ExecutionAlreadyStarted,
            is OwnerIpcCancellation.TerminalReadback,
            OwnerIpcCancellation.Unknown -> Unit
        }
    }

    internal fun sendResponse(
        requestMessage: Message,
        requestId: Long,
        clientSessionId: String?,
        result: ControlServiceResponse,
    ) {
        val response = Message.obtain(null, RESPONSE).apply {
            data = Bundle().apply {
                putSerializable(PROTOCOL_VERSION, AndroidHostProtocolVersion.V10)
                putString(CLIENT_SESSION_ID, clientSessionId)
                putString(OWNER_SESSION_ID, ownerSessionId)
                putLong(REQUEST_ID, requestId)
                putSerializable(RESPONSE_VALUE, result.reply)
                putInt(STATUS_CODE, result.statusCode.value.toInt())
                putString(STATUS_MESSAGE, result.statusMessage)
                putOwnerResponsePayload(result.payload)
            }
        }
        try {
            requestMessage.replyTo?.send(response)
        } catch (error: RemoteException) {
            Log.w(TAG, "host IPC reply was lost: ${error.message}")
        }
    }

    companion object {
        internal const val REQUEST = 41
        internal const val RESPONSE = 42
        internal const val PROTOCOL_VERSION = "protocol_version"
        internal const val CLIENT_SESSION_ID = "client_session_id"
        internal const val EXPECTED_OWNER_SESSION_ID = "expected_owner_session_id"
        internal const val OWNER_SESSION_ID = "owner_session_id"
        internal const val REQUEST_ID = "request_id"
        internal const val REQUEST_DEADLINE_ELAPSED_REALTIME = "request_deadline_elapsed_realtime"
        internal const val CANCEL_START_REQUEST = 43
        internal const val ACK_START_REQUEST = 44
        internal const val CANCEL_OWNER_REQUEST = 45
        internal const val REQUEST_VALUE = "request"
        internal const val RESPONSE_VALUE = "response"
        internal const val RESPONSE_PAYLOAD_KIND = "response_payload_kind"
        internal const val STATUS_CODE = "status_code"
        internal const val STATUS_MESSAGE = "status_message"
        internal const val OWNER_OPERATION_JSON = "owner_operation_json"
        internal const val PROFILE_AGE_CREDENTIAL_METADATA_JSON = "profile_age_credential_metadata_json"
        internal const val PROFILE_AGE_CREDENTIAL_PRIVATE_KEY_BYTES = "profile_age_credential_private_key_bytes"
        internal const val OWNER_DATA_REVISION = "owner_data_revision"
        internal const val OWNER_DATA_GENERATION = "owner_data_generation"
        internal const val OWNER_OPERATION_DATA_FACTS_KIND = "owner_operation_data_facts_kind"
        internal const val OWNER_OPERATION_DATA_DOMAIN = "owner_operation_data_domain"
        internal const val OWNER_OPERATION_DATA_SNAPSHOT_JSON = "owner_operation_data_snapshot_json"
        internal const val OWNER_OPERATION_RESULT_JSON = "owner_operation_result_json"
        internal const val PRODUCT_COMMAND_JSON = "product_command_json"
        internal const val PRODUCT_COMMAND_RESULT_JSON = "product_command_result_json"
        internal const val SURFACE_SNAPSHOT_JSON = "surface_snapshot_json"
        internal const val VPN_CONFIGURATION = "vpn_configuration"
        internal const val VPN_CONFIGURATION_OUTCOME = "vpn_configuration_outcome"
        internal const val VPN_AUTHORIZATION_STATE = "vpn_authorization_state"
        internal const val VPN_START_OUTCOME = "vpn_start_outcome"
        internal const val VPN_OWNER_SNAPSHOT_JSON = "vpn_owner_snapshot_json"
        internal const val VPN_STOP_SNAPSHOT_JSON = "vpn_stop_snapshot_json"
        internal const val VPN_STOP_TARGET_CODE = "vpn_stop_target_code"
        internal const val VPN_STOP_GENERATION = "vpn_stop_generation"
        internal const val CORE_GENERATION = "core_generation"
        internal const val CORE_READBACK_KIND = "core_readback_kind"
        internal const val CORE_SNAPSHOT_LIFECYCLE = "core_snapshot_lifecycle"
        internal const val CORE_SNAPSHOT_GENERATION = "core_snapshot_generation"
        internal const val CORE_SNAPSHOT_SESSION_TOKEN = "core_snapshot_session_token"
        internal const val CORE_SNAPSHOT_REVISION = "core_snapshot_revision"
        internal const val CORE_SNAPSHOT_IDENTITY_JSON = "core_snapshot_identity_json"
        internal const val CORE_SNAPSHOT_FAILURE_JSON = "core_snapshot_failure_json"
        private const val MAX_OWNER_REQUEST_WAITERS = 4
        internal const val MAX_VPN_START_REQUEST_WAITERS = 4
        internal const val TAG = "MihomoControlService"
    }
}

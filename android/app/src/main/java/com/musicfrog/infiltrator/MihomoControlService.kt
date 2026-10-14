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
    private lateinit var commandHandler: Handler
    private lateinit var ownerSessionId: String
    @Volatile
    private var ownerServicePhase: OwnerServicePhase = OwnerServicePhase.Initializing
    private var ownerSessionStartJob: Job? = null
    private lateinit var vpnStartExecutor: ThreadPoolExecutor
    private val vpnStartAdmission = VpnOwnerStartRequestAdmission()
    private val vpnStartTasks = ConcurrentHashMap<VpnOwnerRequestKey, FutureTask<Unit>>()
    private val vpnStartTerminalCache = VpnOwnerStartTerminalCache<OwnerResponse>()
    private val vpnStartRepliesLock = Any()
    private val vpnStartReplies = mutableMapOf<VpnOwnerRequestKey, MutableList<Message>>()
    private val ownerRequestAdmission = OwnerIpcRequestAdmission<OwnerResponse>()
    private val ownerRequestLock = Any()
    private val ownerRequestReplies = mutableMapOf<OwnerIpcRequestKey, MutableList<Message>>()
    private val ownerRequestJobs = mutableMapOf<OwnerIpcRequestKey, Job>()
    private val ownerRequestSensitivePayloads = OwnerIpcSensitivePayloadStore()

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
                            OwnerResponse(
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
                OwnerResponse(AndroidHostReply.FAILED, FfiErrorCode.INVALID_INPUT, "request bundle unavailable"),
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
                OwnerResponse(
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
                OwnerResponse(AndroidHostReply.SERVICE_UNAVAILABLE, FfiErrorCode.NOT_READY)
            }
            request == null -> {
                wipeSensitiveRequest(message, arguments)
                OwnerResponse(AndroidHostReply.FAILED, FfiErrorCode.INVALID_INPUT)
            }
            request == AndroidHostRequest.START_VPN_SERVICE -> {
                val deadline = runCatching { readRequestDeadline(arguments) }.getOrNull()
                if (deadline == null) {
                    wipeSensitiveRequest(message, arguments)
                    OwnerResponse(
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
        fun sendTerminal(response: OwnerResponse) {
            vpnStartTerminalCache.store(key, response, SystemClock.elapsedRealtime())
            sendResponse(message, requestId, clientSessionId, response)
        }
        val admission = vpnStartAdmission.admit(key, deadline, now)
        when (admission) {
            VpnOwnerStartAdmission.Expired -> sendTerminal(
                OwnerResponse(
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
                    OwnerResponse(
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
                        OwnerResponse(
                            AndroidHostReply.FAILED,
                            FfiErrorCode.INVALID_STATE,
                            "VPN start request no longer has an active terminal result",
                        ),
                    )
                }
            }
            VpnOwnerStartAdmission.QueueFull -> sendTerminal(
                OwnerResponse(
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
                    val rejected = OwnerResponse(
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
                OwnerResponse(
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
                OwnerResponse(
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
                        OwnerIpcExecutionStart.Canceled -> OwnerResponse(
                            AndroidHostReply.FAILED,
                            FfiErrorCode.NOT_READY,
                            "owner request was canceled before execution",
                        )
                        OwnerIpcExecutionStart.Expired -> OwnerResponse(
                            AndroidHostReply.SERVICE_UNAVAILABLE,
                            FfiErrorCode.NOT_READY,
                            "owner request expired before execution",
                        )
                        OwnerIpcExecutionStart.InFlight -> OwnerResponse(
                            AndroidHostReply.FAILED,
                            FfiErrorCode.INVALID_STATE,
                            "owner request is already executing",
                        )
                        is OwnerIpcExecutionStart.TerminalReadback -> execution.response
                        OwnerIpcExecutionStart.Unknown -> OwnerResponse(
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
                    OwnerResponse(
                        AndroidHostReply.SERVICE_UNAVAILABLE,
                        FfiErrorCode.NOT_READY,
                        "duplicate owner IPC waiter limit reached",
                    ),
                )
                OwnerIpcWaiterAdmission.NoActiveRequest -> sendResponse(
                    message,
                    requestId,
                    clientSessionId,
                    OwnerResponse(
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
                OwnerResponse(
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
                OwnerResponse(
                    AndroidHostReply.SERVICE_UNAVAILABLE,
                    FfiErrorCode.NOT_READY,
                    "owner IPC request queue is full",
                ),
            )
            OwnerIpcAdmission.Expired -> sendResponse(
                message,
                requestId,
                clientSessionId,
                OwnerResponse(
                    AndroidHostReply.SERVICE_UNAVAILABLE,
                    FfiErrorCode.NOT_READY,
                    "owner request expired before admission",
                ),
            )
            OwnerIpcAdmission.RequestIdConflict -> sendResponse(
                message,
                requestId,
                clientSessionId,
                OwnerResponse(
                    AndroidHostReply.FAILED,
                    FfiErrorCode.INVALID_STATE,
                    "owner request ID was reused with different input",
                ),
            )
        }
    }

    private fun publishOwnerRequest(key: OwnerIpcRequestKey, response: OwnerResponse) {
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

    private fun wipeOwnerRequestSensitivePayload(key: OwnerIpcRequestKey) {
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
                        OwnerResponse(
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

    private fun executeVpnStart(
        key: VpnOwnerRequestKey,
        deadline: VpnOwnerRequestDeadline,
    ) {
        val response = when (val execution = vpnStartAdmission.begin(key, SystemClock.elapsedRealtime())) {
            is VpnOwnerStartExecution.Started -> startVpnService(key, execution.deadline)
            VpnOwnerStartExecution.Canceled -> OwnerResponse(
                AndroidHostReply.VPN_START_TIMED_OUT,
                FfiErrorCode.NOT_READY,
                "VPN start request was canceled before execution",
                OwnerResponsePayload.VpnStart(VpnStartOutcomeWire.TimedOut),
            )
            VpnOwnerStartExecution.Expired -> OwnerResponse(
                AndroidHostReply.VPN_START_TIMED_OUT,
                FfiErrorCode.NOT_READY,
                "VPN start request expired in the owner queue",
                OwnerResponsePayload.VpnStart(VpnStartOutcomeWire.TimedOut),
            )
            VpnOwnerStartExecution.Unknown -> OwnerResponse(
                AndroidHostReply.FAILED,
                FfiErrorCode.INVALID_STATE,
                "VPN start request is no longer admitted",
                OwnerResponsePayload.VpnStart(VpnStartOutcomeWire.Failed),
            )
        }
        vpnStartTerminalCache.store(key, response, SystemClock.elapsedRealtime())
        val completion = vpnStartAdmission.complete(key)
        vpnStartTasks.remove(key)
        scheduleStartExpiry(key, deadline)
        when (completion) {
            VpnOwnerStartCompletion.Publish,
            VpnOwnerStartCompletion.SuppressLateReceipt -> publishVpnStartReplies(key, response)
            VpnOwnerStartCompletion.Unknown -> Log.i(
                TAG,
                "suppressed terminal VPN start receipt for canceled request=${key.requestId}",
            )
        }
    }

    private fun attachVpnStartReply(
        key: VpnOwnerRequestKey,
        message: Message,
    ): VpnStartReplyAdmission = synchronized(vpnStartRepliesLock) {
        val replies = vpnStartReplies[key] ?: return@synchronized VpnStartReplyAdmission.NoActiveRequest
        if (replies.size >= MAX_VPN_START_REQUEST_WAITERS) {
            VpnStartReplyAdmission.CapacityExceeded
        } else {
            replies += message
            VpnStartReplyAdmission.Attached
        }
    }

    private fun publishVpnStartReplies(key: VpnOwnerRequestKey, response: OwnerResponse) {
        val replies = synchronized(vpnStartRepliesLock) {
            vpnStartReplies.remove(key).orEmpty().toList()
        }
        replies.forEach { requestMessage ->
            sendResponse(requestMessage, key.requestId, key.clientSessionId, response)
        }
    }

    private fun clearVpnStartReplies(key: VpnOwnerRequestKey) {
        synchronized(vpnStartRepliesLock) { vpnStartReplies.remove(key) }
    }

    private fun cacheCanceledVpnStart(key: VpnOwnerRequestKey) {
        vpnStartTerminalCache.store(
            key,
            OwnerResponse(
                reply = AndroidHostReply.VPN_START_TIMED_OUT,
                statusCode = FfiErrorCode.NOT_READY,
                statusMessage = "VPN start request was canceled before service dispatch",
                payload = OwnerResponsePayload.VpnStart(VpnStartOutcomeWire.TimedOut),
            ),
            SystemClock.elapsedRealtime(),
        )
    }

    private fun cancelStartRequest(message: Message) {
        val arguments = runCatching { message.data }.getOrNull() ?: return
        arguments.classLoader = AndroidHostProtocolVersion::class.java.classLoader
        val protocol = readProtocolVersion(arguments)
        val clientSessionId = arguments.getString(CLIENT_SESSION_ID)
        val requestId = arguments.getLong(REQUEST_ID, Long.MIN_VALUE)
        if (protocol != AndroidHostProtocolVersion.V10 || clientSessionId.isNullOrBlank() || requestId <= 0L) {
            Log.w(TAG, "ignored malformed VPN start cancellation")
            return
        }
        val key = VpnOwnerRequestKey(clientSessionId, requestId)
        when (val cancellation = vpnStartAdmission.cancel(key)) {
            VpnOwnerStartCancellation.CanceledBeforeGeneration -> {
                clearVpnStartReplies(key)
                cacheCanceledVpnStart(key)
                val task = vpnStartTasks[key]
                if (task != null && task.cancel(false)) {
                    vpnStartExecutor.remove(task)
                    vpnStartTasks.remove(key)
                    vpnStartAdmission.complete(key)
                }
            }
            is VpnOwnerStartCancellation.CancelGeneration -> {
                clearVpnStartReplies(key)
                cacheCanceledVpnStart(key)
                MihomoVpnService.cancelUndispatchedStart(cancellation.generation)
            }
            VpnOwnerStartCancellation.ExecutionAlreadyStarted -> {
                clearVpnStartReplies(key)
                Log.i(
                    TAG,
                    "VPN start request already crossed service dispatch: request=${key.requestId}",
                )
            }
            VpnOwnerStartCancellation.TerminalReadback -> Log.i(
                TAG,
                "VPN start request already reached a terminal result: request=${key.requestId}",
            )
            VpnOwnerStartCancellation.AlreadyCanceled,
            VpnOwnerStartCancellation.Unknown -> Unit
        }
    }

    private fun acknowledgeStartRequest(message: Message) {
        val arguments = runCatching { message.data }.getOrNull() ?: return
        arguments.classLoader = AndroidHostProtocolVersion::class.java.classLoader
        val protocol = readProtocolVersion(arguments)
        val clientSessionId = arguments.getString(CLIENT_SESSION_ID)
        val requestId = arguments.getLong(REQUEST_ID, Long.MIN_VALUE)
        if (protocol != AndroidHostProtocolVersion.V10 || clientSessionId.isNullOrBlank() || requestId <= 0L) {
            Log.w(TAG, "ignored malformed VPN start acknowledgment")
            return
        }
        vpnStartAdmission.acknowledgeTerminal(VpnOwnerRequestKey(clientSessionId, requestId))
    }

    private fun scheduleStartExpiry(key: VpnOwnerRequestKey, deadline: VpnOwnerRequestDeadline) {
        val remainingMillis = when (val budget = deadline.remainingAt(SystemClock.elapsedRealtime())) {
            VpnDeadlineState.Expired -> 0L
            is VpnDeadlineState.Remaining -> budget.millis
        }
        commandHandler.postDelayed({
            when (val expiry = vpnStartAdmission.expireTerminal(key, SystemClock.elapsedRealtime())) {
                VpnOwnerStartExpiry.NotExpired -> scheduleStartExpiry(key, deadline)
                VpnOwnerStartExpiry.ExpiredTerminal,
                VpnOwnerStartExpiry.Unknown -> Unit
            }
        }, remainingMillis)
    }

    private fun sendResponse(
        requestMessage: Message,
        requestId: Long,
        clientSessionId: String?,
        result: OwnerResponse,
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

    private fun Bundle.putOwnerResponsePayload(payload: OwnerResponsePayload) {
        putSerializable(RESPONSE_PAYLOAD_KIND, payload.kind())
        when (payload) {
            OwnerResponsePayload.Failure -> Unit
            is OwnerResponsePayload.CoreLifecycleSnapshot -> putCoreSnapshot(payload.snapshot)
            is OwnerResponsePayload.VpnOwnerSnapshot -> putString(
                VPN_OWNER_SNAPSHOT_JSON,
                payload.canonicalJson,
            )
            is OwnerResponsePayload.SurfaceSnapshot -> {
                putString(SURFACE_SNAPSHOT_JSON, payload.json)
                putCoreReadback(payload.coreReadback)
            }
            is OwnerResponsePayload.OwnerDataRevision -> {
                putLong(OWNER_DATA_REVISION, payload.revision)
                putLong(OWNER_DATA_GENERATION, payload.generation)
                putCoreReadback(payload.coreReadback)
            }
            is OwnerResponsePayload.Authorization -> putSerializable(
                VPN_AUTHORIZATION_STATE,
                payload.state,
            )
            is OwnerResponsePayload.ProductCommand -> {
                putString(PRODUCT_COMMAND_RESULT_JSON, payload.resultJson)
                putCoreReadback(payload.coreReadback)
            }
            is OwnerResponsePayload.OwnerOperation -> {
                putString(OWNER_OPERATION_RESULT_JSON, payload.outcomeJson)
                putLong(OWNER_DATA_REVISION, payload.revision)
                putLong(OWNER_DATA_GENERATION, payload.generation)
                when (val facts = payload.dataFacts) {
                    OwnerOperationDataFacts.RevisionOnly -> putSerializable(
                        OWNER_OPERATION_DATA_FACTS_KIND,
                        OwnerOperationDataFactsKind.RevisionOnly,
                    )
                    is OwnerOperationDataFacts.Domain -> {
                        putSerializable(
                            OWNER_OPERATION_DATA_FACTS_KIND,
                            OwnerOperationDataFactsKind.Domain,
                        )
                        putSerializable(OWNER_OPERATION_DATA_DOMAIN, facts.domain)
                    }
                    is OwnerOperationDataFacts.Snapshot -> {
                        putSerializable(
                            OWNER_OPERATION_DATA_FACTS_KIND,
                            OwnerOperationDataFactsKind.Snapshot,
                        )
                        putSerializable(OWNER_OPERATION_DATA_DOMAIN, facts.domain)
                        putString(OWNER_OPERATION_DATA_SNAPSHOT_JSON, facts.json)
                    }
                }
                putCoreReadback(payload.coreReadback)
            }
            is OwnerResponsePayload.StopVpn -> putString(VPN_STOP_SNAPSHOT_JSON, payload.snapshotJson)
            is OwnerResponsePayload.VpnStart -> putSerializable(VPN_START_OUTCOME, payload.outcome)
            is OwnerResponsePayload.VpnConfiguration -> putSerializable(
                VPN_CONFIGURATION_OUTCOME,
                payload.outcome,
            )
            is OwnerResponsePayload.OwnerSessionReadback -> {
                putLong(OWNER_DATA_REVISION, payload.ownerDataRevision)
                putCoreReadback(payload.coreReadback)
            }
        }
    }

    private fun Bundle.putCoreReadback(readback: OwnerCoreReadback) {
        when (readback) {
            OwnerCoreReadback.Unavailable -> putSerializable(
                CORE_READBACK_KIND,
                OwnerCoreReadbackKind.Unavailable,
            )
            is OwnerCoreReadback.Available -> {
                putSerializable(CORE_READBACK_KIND, OwnerCoreReadbackKind.Available)
                putCoreSnapshot(readback.snapshot)
            }
        }
    }

    private fun Bundle.putCoreSnapshot(snapshot: OwnerCoreSnapshot) {
        putSerializable(CORE_SNAPSHOT_LIFECYCLE, snapshot.lifecycle)
        putLong(CORE_SNAPSHOT_GENERATION, snapshot.generation)
        putString(CORE_SNAPSHOT_SESSION_TOKEN, snapshot.sessionToken)
        putLong(CORE_SNAPSHOT_REVISION, snapshot.revision)
        putString(CORE_SNAPSHOT_IDENTITY_JSON, snapshot.identity.toJsonValue().toString())
        putString(CORE_SNAPSHOT_FAILURE_JSON, snapshot.failure?.toJson()?.toString())
    }

    private suspend fun executeAdmitted(
        key: OwnerIpcRequestKey,
        request: AndroidHostRequest,
        arguments: Bundle,
        expectedOwnerSession: String?,
    ): OwnerResponse {
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

    private suspend fun execute(
        request: AndroidHostRequest,
        arguments: Bundle,
        requestKey: OwnerIpcRequestKey,
    ): OwnerResponse {
        val host = (application as? MihomoApplication)?.coreHost
            ?: return OwnerResponse(
                AndroidHostReply.SERVICE_UNAVAILABLE,
                FfiErrorCode.NOT_READY,
            )
        return when (request) {
            AndroidHostRequest.READ_CORE_LIFECYCLE -> readCoreLifecycle()
            AndroidHostRequest.EXECUTE_PRODUCT_COMMAND -> {
                val commandJson = arguments.getString(PRODUCT_COMMAND_JSON)
                    ?: return OwnerResponse(AndroidHostReply.FAILED, FfiErrorCode.INVALID_INPUT)
                val resultJson = androidExecuteProductCommandJson(commandJson)
                OwnerResponse(
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
                    OwnerResponse(
                        AndroidHostReply.SURFACE_SNAPSHOT_AVAILABLE,
                        FfiErrorCode.OK,
                        payload = OwnerResponsePayload.SurfaceSnapshot(
                            snapshotJson,
                            OwnerCoreReadback.Unavailable,
                        ),
                    ).withCurrentCoreSnapshot()
                } else {
                    OwnerResponse(
                        AndroidHostReply.FAILED,
                        result.status.code,
                        result.status.message,
                    )
                }
            }
            AndroidHostRequest.READ_OWNER_DATA_REVISION ->
                OwnerResponse(
                    AndroidHostReply.OWNER_DATA_REVISION_AVAILABLE,
                    FfiErrorCode.OK,
                    payload = OwnerResponsePayload.OwnerDataRevision(
                        androidOwnerDataRevision().toLong(),
                        generation = 0L,
                        coreReadback = OwnerCoreReadback.Unavailable,
                    ),
                ).withCurrentCoreSnapshot()
            AndroidHostRequest.EXECUTE_OWNER_OPERATION -> {
                val operationJson = arguments.getString(OWNER_OPERATION_JSON)
                    ?: return OwnerResponse(AndroidHostReply.FAILED, FfiErrorCode.INVALID_INPUT)
                val expectedOwnerSession = arguments.getString(EXPECTED_OWNER_SESSION_ID)
                    ?: return OwnerResponse(AndroidHostReply.FAILED, FfiErrorCode.NOT_READY)
                val result = androidDispatchOwnerOperationWireJson(
                    operationJson,
                    arguments.getString(CLIENT_SESSION_ID).orEmpty(),
                    expectedOwnerSession,
                    ownerSessionId,
                    arguments.getLong(CORE_GENERATION).toULong(),
                    arguments.getLong(OWNER_DATA_REVISION).toULong(),
                )
                val reply = when (result.state) {
                    AndroidOwnerOperationIpcState.COMPLETED -> AndroidHostReply.OWNER_OPERATION_COMPLETED
                    AndroidOwnerOperationIpcState.STALE_OWNER_DATA -> AndroidHostReply.STALE_OWNER_DATA
                    AndroidOwnerOperationIpcState.VPN_CONFIGURATION_BUSY -> AndroidHostReply.VPN_CONFIGURATION_BUSY
                    AndroidOwnerOperationIpcState.FAILED -> AndroidHostReply.FAILED
                }
                val dataDomain = result.dataDomain
                val snapshotJson = result.snapshotJson
                OwnerResponse(
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
                    arguments.getString(PROFILE_AGE_CREDENTIAL_METADATA_JSON)
                }.getOrNull()
                    ?: return OwnerResponse(AndroidHostReply.FAILED, FfiErrorCode.INVALID_INPUT)
                val privateKeyBytes = when (
                    val payload = ownerRequestSensitivePayloads.lookup(requestKey)
                ) {
                    OwnerIpcSensitivePayloadLookup.Missing -> return OwnerResponse(
                        AndroidHostReply.FAILED,
                        FfiErrorCode.INVALID_INPUT,
                        "Age credential request has no one-shot secret payload",
                    )
                    is OwnerIpcSensitivePayloadLookup.Present -> payload.bytes
                }
                val expectedOwnerSession = runCatching {
                    arguments.getString(EXPECTED_OWNER_SESSION_ID)
                }.getOrNull()
                    ?: run {
                        privateKeyBytes.fill(0)
                        return OwnerResponse(AndroidHostReply.FAILED, FfiErrorCode.NOT_READY)
                    }
                val expectedCoreGeneration = runCatching {
                    arguments.getLong(CORE_GENERATION, -1L)
                }.getOrDefault(-1L)
                val expectedOwnerDataRevision = runCatching {
                    arguments.getLong(OWNER_DATA_REVISION, -1L)
                }.getOrDefault(-1L)
                if (expectedCoreGeneration < 0L || expectedOwnerDataRevision < 0L) {
                    privateKeyBytes.fill(0)
                    return OwnerResponse(
                        AndroidHostReply.FAILED,
                        FfiErrorCode.INVALID_INPUT,
                        "Age credential request omitted its owner revision fence",
                    )
                }
                val result = try {
                    androidDispatchProfileAgeCredentialOwnerOperationWireJson(
                        metadataJson,
                        privateKeyBytes,
                        runCatching { arguments.getString(CLIENT_SESSION_ID) }
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
                OwnerResponse(
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
            AndroidHostRequest.START_VPN_SERVICE -> OwnerResponse(
                AndroidHostReply.SERVICE_UNAVAILABLE,
                FfiErrorCode.NOT_READY,
                "VPN start must use bounded owner admission",
            )
            AndroidHostRequest.STOP_VPN_SERVICE -> stopVpn(arguments)
            AndroidHostRequest.READ_VPN_AUTHORIZATION -> {
                val authorization = readVpnAuthorization()
                OwnerResponse(
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

    private suspend fun readVpnOwnerSnapshot(): OwnerResponse {
        val snapshot = currentVpnOwnerSnapshotJson()
            ?: return OwnerResponse(
                AndroidHostReply.SERVICE_UNAVAILABLE,
                FfiErrorCode.NOT_READY,
                "VPN owner snapshot is unavailable",
            )
        return OwnerResponse(
            reply = AndroidHostReply.VPN_OWNER_SNAPSHOT_AVAILABLE,
            statusCode = FfiErrorCode.OK,
            payload = OwnerResponsePayload.VpnOwnerSnapshot(snapshot),
        )
    }

    private suspend fun stopVpn(arguments: Bundle): OwnerResponse {
        val deadline = readRequestDeadline(arguments)
        if (deadline == null) {
            val snapshot = currentVpnOwnerSnapshotJson()
            return OwnerResponse(
                reply = AndroidHostReply.VPN_STOP_TIMED_OUT,
                statusCode = FfiErrorCode.NOT_READY,
                statusMessage = "VPN stop request omitted its absolute deadline",
                payload = snapshot?.let { OwnerResponsePayload.StopVpn(it) }
                    ?: OwnerResponsePayload.Failure,
            )
        }
        if (!arguments.containsKey(VPN_STOP_TARGET_CODE) ||
            !arguments.containsKey(VPN_STOP_GENERATION)
        ) {
            val snapshot = currentVpnOwnerSnapshotJson()
            return OwnerResponse(
                AndroidHostReply.FAILED,
                FfiErrorCode.INVALID_INPUT,
                "VPN stop request omitted its typed target",
                payload = snapshot?.let { OwnerResponsePayload.StopVpn(it) }
                    ?: OwnerResponsePayload.Failure,
            )
        }
        val target = VpnStopTarget.fromWire(
            arguments.getInt(VPN_STOP_TARGET_CODE),
            arguments.getLong(VPN_STOP_GENERATION),
        ) ?: return OwnerResponse(
            reply = AndroidHostReply.FAILED,
            statusCode = FfiErrorCode.INVALID_INPUT,
            statusMessage = "VPN stop request carried an invalid target",
            payload = currentVpnOwnerSnapshotJson()?.let { OwnerResponsePayload.StopVpn(it) }
                ?: OwnerResponsePayload.Failure,
        )
        when (target) {
            VpnStopTarget.Current -> Unit
            is VpnStopTarget.Generation -> return OwnerResponse(
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
                true -> OwnerResponse(
                    AndroidHostReply.VPN_STOPPED,
                    FfiErrorCode.OK,
                )
                false, null -> OwnerResponse(
                    AndroidHostReply.VPN_STOP_SUPERSEDED,
                    FfiErrorCode.INVALID_STATE,
                    "VPN stop was superseded before owner readback",
                )
            }
            is VpnStopResult.Failed -> {
                Log.e(TAG, "VpnService shutdown failed: ${result.message}")
                OwnerResponse(
                    AndroidHostReply.FAILED,
                    FfiErrorCode.INVALID_STATE,
                    result.message,
                )
            }
            VpnStopResult.TimedOut -> OwnerResponse(
                AndroidHostReply.VPN_STOP_TIMED_OUT,
                FfiErrorCode.NOT_READY,
                "VpnService did not stop before the shutdown deadline",
            )
            VpnStopResult.Superseded -> OwnerResponse(
                AndroidHostReply.VPN_STOP_SUPERSEDED,
                FfiErrorCode.INVALID_STATE,
                "VPN stop request belongs to an older generation",
            )
            VpnStopResult.Canceled -> OwnerResponse(
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

    private suspend fun currentVpnOwnerSnapshotJson(): String? =
        (application as? MihomoApplication)?.refreshAndPublishVpnOwnerSnapshotJson()

    /**
     * A request carrying the retired process session is rejected. The client
     * treats this Core-only reply as partial and obtains one complete VPN/Core
     * owner snapshot before it issues another operation.
     */
    private suspend fun recoverOwnerSession(): OwnerResponse {
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

    private suspend fun OwnerResponse.withCurrentCoreSnapshot(): OwnerResponse {
        val readback = androidCoreLifecycleStatus().toOwnerCoreReadback()
        return copy(payload = payload.withCoreReadback(readback))
    }

    private suspend fun readCoreLifecycle(): OwnerResponse {
        val result = androidCoreLifecycleStatus()
        return when {
            result.status.code != FfiErrorCode.OK -> result.toOwnerResponse(AndroidHostReply.FAILED)
            else -> when (val core = result.toOwnerCoreReadback()) {
                OwnerCoreReadback.Unavailable -> OwnerResponse(
                    AndroidHostReply.FAILED,
                    FfiErrorCode.NOT_READY,
                    "Core lifecycle snapshot is unavailable",
                )
                is OwnerCoreReadback.Available -> OwnerResponse(
                    AndroidHostReply.CORE_LIFECYCLE_AVAILABLE,
                    FfiErrorCode.OK,
                    payload = OwnerResponsePayload.CoreLifecycleSnapshot(core.snapshot),
                )
            }
        }
    }

    private fun applyVpnConfiguration(arguments: Bundle): OwnerResponse {
        val configuration = arguments.getString(VPN_CONFIGURATION)
        if (configuration == null) {
            return OwnerResponse(AndroidHostReply.FAILED, FfiErrorCode.INVALID_INPUT)
        }
        return when (MihomoVpnService.setPendingConfiguration(this, configuration)) {
            VpnConfigurationApplyOutcomeWire.Accepted ->
                OwnerResponse(
                    AndroidHostReply.VPN_CONFIGURATION_ACCEPTED,
                    FfiErrorCode.OK,
                    payload = OwnerResponsePayload.VpnConfiguration(
                        VpnConfigurationApplyOutcomeWire.Accepted,
                    ),
                )
            VpnConfigurationApplyOutcomeWire.Rejected ->
                OwnerResponse(
                    AndroidHostReply.FAILED,
                    FfiErrorCode.INVALID_INPUT,
                    payload = OwnerResponsePayload.VpnConfiguration(
                        VpnConfigurationApplyOutcomeWire.Rejected,
                    ),
                )
            VpnConfigurationApplyOutcomeWire.OwnerUnavailable ->
                OwnerResponse(
                    AndroidHostReply.SERVICE_UNAVAILABLE,
                    FfiErrorCode.NOT_READY,
                    payload = OwnerResponsePayload.VpnConfiguration(
                        VpnConfigurationApplyOutcomeWire.OwnerUnavailable,
                    ),
                )
        }
    }

    private fun startVpnService(
        key: VpnOwnerRequestKey,
        deadline: VpnOwnerRequestDeadline,
    ): OwnerResponse {
        when (val authorization = readVpnAuthorization()) {
            OwnerVpnAuthorization.PermissionRequired -> {
                VpnAuthorizationBroadcast.requestForExplicitStart(this)
                return OwnerResponse(
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
            VpnStartResult.Running -> OwnerResponse(
                AndroidHostReply.VPN_RUNNING,
                FfiErrorCode.OK,
                payload = OwnerResponsePayload.VpnStart(outcome),
            )
            VpnStartResult.PermissionRequired -> OwnerResponse(
                AndroidHostReply.VPN_PERMISSION_REQUIRED,
                FfiErrorCode.OK,
                payload = OwnerResponsePayload.VpnStart(outcome),
            )
            is VpnStartResult.Failed -> {
                Log.e(TAG, "VpnService startup failed: ${result.message}")
                OwnerResponse(
                    AndroidHostReply.FAILED,
                    FfiErrorCode.INVALID_STATE,
                    result.message,
                    OwnerResponsePayload.VpnStart(outcome),
                )
            }
            VpnStartResult.TimedOut -> {
                Log.e(TAG, "VpnService startup timed out before proxy data path was running")
                OwnerResponse(
                    AndroidHostReply.VPN_START_TIMED_OUT,
                    FfiErrorCode.NOT_READY,
                    payload = OwnerResponsePayload.VpnStart(outcome),
                )
            }
        }
    }

    private fun readVpnAuthorization(): OwnerVpnAuthorization {
        return when (VpnService.prepare(this)) {
            null -> OwnerVpnAuthorization.Authorized
            else -> OwnerVpnAuthorization.PermissionRequired
        }
    }

    private fun readRequest(bundle: Bundle): AndroidHostRequest? {
        bundle.classLoader = AndroidHostRequest::class.java.classLoader
        return if (android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.TIRAMISU) {
            bundle.getSerializable(REQUEST_VALUE, AndroidHostRequest::class.java)
        } else {
            @Suppress("DEPRECATION")
            bundle.getSerializable(REQUEST_VALUE) as? AndroidHostRequest
        }
    }

    private fun readProtocolVersion(bundle: Bundle): AndroidHostProtocolVersion? {
        bundle.classLoader = AndroidHostProtocolVersion::class.java.classLoader
        return if (android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.TIRAMISU) {
            bundle.getSerializable(PROTOCOL_VERSION, AndroidHostProtocolVersion::class.java)
        } else {
            @Suppress("DEPRECATION")
            bundle.getSerializable(PROTOCOL_VERSION) as? AndroidHostProtocolVersion
        }
    }

    private fun readRequestDeadline(bundle: Bundle): VpnOwnerRequestDeadline? {
        if (!bundle.containsKey(REQUEST_DEADLINE_ELAPSED_REALTIME)) return null
        val deadline = bundle.getLong(REQUEST_DEADLINE_ELAPSED_REALTIME, 0L)
        return deadline.takeIf { it > 0L }?.let(::VpnOwnerRequestDeadline)
    }

    private fun AndroidCoreLifecycleResult.toOwnerResponse(accepted: AndroidHostReply): OwnerResponse =
        when (val core = toOwnerCoreReadback()) {
            OwnerCoreReadback.Unavailable -> OwnerResponse(
                accepted,
                status.code,
                status.message,
            )
            is OwnerCoreReadback.Available -> OwnerResponse(
                accepted,
                status.code,
                status.message,
                OwnerResponsePayload.CoreLifecycleSnapshot(core.snapshot),
            )
        }

    private fun AndroidCoreLifecycleResult.toOwnerCoreReadback(): OwnerCoreReadback {
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

    private fun infiltrator_android.FfiStatus.toOwnerResponse(accepted: AndroidHostReply): OwnerResponse =
        OwnerResponse(
            if (code == FfiErrorCode.OK) accepted else AndroidHostReply.FAILED,
            code,
            message,
        )

    private data class OwnerResponse(
        val reply: AndroidHostReply,
        val statusCode: FfiErrorCode,
        val statusMessage: String? = null,
        val payload: OwnerResponsePayload = OwnerResponsePayload.Failure,
    )

    private enum class OwnerIpcWaiterAdmission {
        Attached,
        CapacityExceeded,
        NoActiveRequest,
    }

    private enum class VpnStartReplyAdmission {
        Attached,
        CapacityExceeded,
        NoActiveRequest,
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
        private const val MAX_VPN_START_REQUEST_WAITERS = 4
        private const val TAG = "MihomoControlService"
    }
}

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
import infiltrator_android.androidExecuteProductCommandJson
import infiltrator_android.androidOwnerDataRevision
import infiltrator_android.androidSurfaceSnapshotJson
import kotlinx.coroutines.CoroutineStart
import kotlinx.coroutines.Job
import kotlinx.coroutines.launch
import java.util.UUID
import java.util.concurrent.ArrayBlockingQueue
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.FutureTask
import java.util.concurrent.RejectedExecutionException
import java.util.concurrent.ThreadPoolExecutor
import java.util.concurrent.TimeUnit

internal fun MihomoControlService.executeVpnStart(key: VpnOwnerRequestKey, deadline: VpnOwnerRequestDeadline) {
    val response = when (val execution = vpnStartAdmission.begin(key, SystemClock.elapsedRealtime())) {
        is VpnOwnerStartExecution.Started -> startVpnService(key, execution.deadline)

        VpnOwnerStartExecution.Canceled -> ControlServiceResponse(
            AndroidHostReply.VPN_START_TIMED_OUT,
            FfiErrorCode.NOT_READY,
            "VPN start request was canceled before execution",
            OwnerResponsePayload.VpnStart(VpnStartOutcomeWire.TimedOut),
        )

        VpnOwnerStartExecution.Expired -> ControlServiceResponse(
            AndroidHostReply.VPN_START_TIMED_OUT,
            FfiErrorCode.NOT_READY,
            "VPN start request expired in the owner queue",
            OwnerResponsePayload.VpnStart(VpnStartOutcomeWire.TimedOut),
        )

        VpnOwnerStartExecution.Unknown -> ControlServiceResponse(
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
        VpnOwnerStartCompletion.SuppressLateReceipt,
        -> publishVpnStartReplies(key, response)

        VpnOwnerStartCompletion.Unknown -> Log.i(
            MihomoControlService.TAG,
            "suppressed terminal VPN start receipt for canceled request=${key.requestId}",
        )
    }
}

internal fun MihomoControlService.attachVpnStartReply(key: VpnOwnerRequestKey, message: Message): VpnStartReplyAdmission =
    synchronized(vpnStartRepliesLock) {
        val replies = vpnStartReplies[key] ?: return@synchronized VpnStartReplyAdmission.NoActiveRequest
        if (replies.size >= MihomoControlService.MAX_VPN_START_REQUEST_WAITERS) {
            VpnStartReplyAdmission.CapacityExceeded
        } else {
            replies += message
            VpnStartReplyAdmission.Attached
        }
    }

internal fun MihomoControlService.publishVpnStartReplies(key: VpnOwnerRequestKey, response: ControlServiceResponse) {
    val replies = synchronized(vpnStartRepliesLock) {
        vpnStartReplies.remove(key).orEmpty().toList()
    }
    replies.forEach { requestMessage ->
        sendResponse(requestMessage, key.requestId, key.clientSessionId, response)
    }
}

internal fun MihomoControlService.clearVpnStartReplies(key: VpnOwnerRequestKey) {
    synchronized(vpnStartRepliesLock) { vpnStartReplies.remove(key) }
}

internal fun MihomoControlService.cacheCanceledVpnStart(key: VpnOwnerRequestKey) {
    vpnStartTerminalCache.store(
        key,
        ControlServiceResponse(
            reply = AndroidHostReply.VPN_START_TIMED_OUT,
            statusCode = FfiErrorCode.NOT_READY,
            statusMessage = "VPN start request was canceled before service dispatch",
            payload = OwnerResponsePayload.VpnStart(VpnStartOutcomeWire.TimedOut),
        ),
        SystemClock.elapsedRealtime(),
    )
}

internal fun MihomoControlService.cancelStartRequest(message: Message) {
    val arguments = runCatching { message.data }.getOrNull() ?: return
    arguments.classLoader = AndroidHostProtocolVersion::class.java.classLoader
    val protocol = readProtocolVersion(arguments)
    val clientSessionId = arguments.getString(MihomoControlService.CLIENT_SESSION_ID)
    val requestId = arguments.getLong(MihomoControlService.REQUEST_ID, Long.MIN_VALUE)
    if (protocol != AndroidHostProtocolVersion.V10 || clientSessionId.isNullOrBlank() || requestId <= 0L) {
        Log.w(MihomoControlService.TAG, "ignored malformed VPN start cancellation")
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
                MihomoControlService.TAG,
                "VPN start request already crossed service dispatch: request=${key.requestId}",
            )
        }

        VpnOwnerStartCancellation.TerminalReadback -> Log.i(
            MihomoControlService.TAG,
            "VPN start request already reached a terminal result: request=${key.requestId}",
        )

        VpnOwnerStartCancellation.AlreadyCanceled,
        VpnOwnerStartCancellation.Unknown,
        -> Unit
    }
}

internal fun MihomoControlService.acknowledgeStartRequest(message: Message) {
    val arguments = runCatching { message.data }.getOrNull() ?: return
    arguments.classLoader = AndroidHostProtocolVersion::class.java.classLoader
    val protocol = readProtocolVersion(arguments)
    val clientSessionId = arguments.getString(MihomoControlService.CLIENT_SESSION_ID)
    val requestId = arguments.getLong(MihomoControlService.REQUEST_ID, Long.MIN_VALUE)
    if (protocol != AndroidHostProtocolVersion.V10 || clientSessionId.isNullOrBlank() || requestId <= 0L) {
        Log.w(MihomoControlService.TAG, "ignored malformed VPN start acknowledgment")
        return
    }
    vpnStartAdmission.acknowledgeTerminal(VpnOwnerRequestKey(clientSessionId, requestId))
}

internal fun MihomoControlService.scheduleStartExpiry(key: VpnOwnerRequestKey, deadline: VpnOwnerRequestDeadline) {
    val remainingMillis = when (val budget = deadline.remainingAt(SystemClock.elapsedRealtime())) {
        VpnDeadlineState.Expired -> 0L
        is VpnDeadlineState.Remaining -> budget.millis
    }
    commandHandler.postDelayed({
        when (vpnStartAdmission.expireTerminal(key, SystemClock.elapsedRealtime())) {
            VpnOwnerStartExpiry.NotExpired -> scheduleStartExpiry(key, deadline)

            VpnOwnerStartExpiry.ExpiredTerminal,
            VpnOwnerStartExpiry.Unknown,
            -> Unit
        }
    }, remainingMillis)
}

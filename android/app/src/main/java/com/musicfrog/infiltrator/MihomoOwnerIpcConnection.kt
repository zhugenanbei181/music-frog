package com.musicfrog.infiltrator

import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.content.ServiceConnection
import android.os.Bundle
import android.os.Handler
import android.os.HandlerThread
import android.os.IBinder
import android.os.Message
import android.os.Messenger
import android.os.RemoteException
import android.os.SystemClock
import android.util.Log
import infiltrator_android.AndroidHostProtocolVersion
import infiltrator_android.AndroidHostRequest
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicReference

internal sealed interface OwnerIpcTransportResult<out T> {
    data class Completed<T>(val response: T) : OwnerIpcTransportResult<T>

    data object PendingCapacityExceeded : OwnerIpcTransportResult<Nothing>

    data object RequestIdAlreadyPending : OwnerIpcTransportResult<Nothing>
}

/** Application-owned Binder binding, reply Handler and bounded request table. */
internal class MihomoOwnerIpcConnection(
    context: Context,
) {
    private val applicationContext = context.applicationContext
    private val lock = Any()
    private val pending = linkedMapOf<Long, Pending<*>>()
    private val pendingLedger = OwnerIpcClientPendingLedger(MAX_PENDING_REQUESTS)
    private var bindingState: BindingState = BindingState.Unbound
    private var nextBindingId = 1L
    private val replyThread = HandlerThread("mihomo-control-client-replies").also { it.start() }
    private val replyMessenger = Messenger(
        Handler(replyThread.looper, Handler.Callback(::handleReply)),
    )

    fun <T> transact(
        request: AndroidHostRequest,
        requestId: Long,
        deadline: VpnOwnerRequestDeadline,
        message: Message,
        decodeReply: (Message) -> T,
        shouldAcknowledge: (T) -> Boolean,
        onFailure: (OwnerClientTerminal) -> T,
        onCancel: (Messenger) -> Unit,
        onAcknowledge: (Messenger) -> Unit,
    ): OwnerIpcTransportResult<T> {
        val pendingRequest = Pending(
            request = request,
            requestId = requestId,
            deadline = deadline,
            message = message,
            decodeReply = decodeReply,
            shouldAcknowledge = shouldAcknowledge,
            onFailure = onFailure,
            onCancel = onCancel,
            onAcknowledge = onAcknowledge,
        )
        if (deadline.remainingAt(SystemClock.elapsedRealtime()) == VpnDeadlineState.Expired) {
            return OwnerIpcTransportResult.Completed(onFailure(OwnerClientTerminal.ReplyTimedOut))
        }

        var bindConnection: ServiceConnection? = null
        var connectedMessenger: Messenger? = null
        val admission = synchronized(lock) {
            when {
                bindingState == BindingState.Closed -> PendingAdmission.Closed
                else -> {
                    when (pendingLedger.admit(requestId)) {
                        OwnerIpcPendingAdmission.CapacityExceeded -> PendingAdmission.Full
                        OwnerIpcPendingAdmission.DuplicateRequestId -> PendingAdmission.Duplicate
                        OwnerIpcPendingAdmission.Admitted -> {
                            pending[requestId] = pendingRequest
                            when (val state = bindingState) {
                                BindingState.Unbound -> {
                                    val bindingId = nextBindingId++
                                    val connection = createConnection(bindingId)
                                    bindingState = BindingState.Binding(bindingId, connection)
                                    bindConnection = connection
                                }
                                is BindingState.Binding -> Unit
                                is BindingState.Connected -> connectedMessenger = state.messenger
                                BindingState.Closed -> Unit
                            }
                            PendingAdmission.Added
                        }
                    }
                }
            }
        }
        when (admission) {
            PendingAdmission.Full -> return OwnerIpcTransportResult.PendingCapacityExceeded
            PendingAdmission.Duplicate -> return OwnerIpcTransportResult.RequestIdAlreadyPending
            PendingAdmission.Closed -> return OwnerIpcTransportResult.Completed(
                onFailure(OwnerClientTerminal.OwnerUnavailable),
            )
            PendingAdmission.Added -> Unit
        }

        val connectionToBind = bindConnection
        if (connectionToBind != null) {
            val acceptance = try {
                OwnerBindAcceptance.fromPlatformResult(
                    applicationContext.bindService(
                        Intent(applicationContext, MihomoControlService::class.java),
                        connectionToBind,
                        Context.BIND_AUTO_CREATE,
                    ),
                )
            } catch (error: Exception) {
                Log.w(TAG, "owner service bind failed: ${error.message}")
                OwnerBindAcceptance.Rejected
            }
            when (acceptance) {
                OwnerBindAcceptance.Accepted -> Unit
                OwnerBindAcceptance.Rejected ->
                    breakBinding(connectionToBind, OwnerClientTerminal.BindRejected)
            }
        } else if (connectedMessenger != null) {
            dispatch(pendingRequest, requireNotNull(connectedMessenger))
        }

        return pendingRequest.awaitResult()
    }

    fun close() {
        val snapshot = synchronized(lock) {
            val current = bindingState
            bindingState = BindingState.Closed
            val requests = pending.values.toList().also { pending.clear() }
            pendingLedger.disconnect(OwnerClientTerminal.OwnerUnavailable)
            when (current) {
                BindingState.Unbound,
                BindingState.Closed -> CloseSnapshot(null, null, null, requests)
                is BindingState.Binding -> CloseSnapshot(current.connection, null, null, requests)
                is BindingState.Connected -> CloseSnapshot(
                    current.connection,
                    current.messenger,
                    current.deathRecipient,
                    requests,
                )
            }
        }
        snapshot.messenger?.binder?.let { binder ->
            snapshot.deathRecipient?.let { recipient -> runCatching { binder.unlinkToDeath(recipient, 0) } }
        }
        snapshot.connection?.let { connectionToUnbind ->
            runCatching { applicationContext.unbindService(connectionToUnbind) }
        }
        snapshot.pendingRequests.forEach { it.fail(OwnerClientTerminal.OwnerUnavailable) }
        replyThread.quitSafely()
    }

    private fun createConnection(bindingId: Long): ServiceConnection = object : ServiceConnection {
        override fun onServiceConnected(name: ComponentName?, service: IBinder?) {
            if (service == null) {
                breakBinding(this, OwnerClientTerminal.NullBinding)
                return
            }
            val deathRecipient = IBinder.DeathRecipient {
                breakBinding(this, OwnerClientTerminal.BinderDied)
            }
            try {
                service.linkToDeath(deathRecipient, 0)
            } catch (error: RemoteException) {
                Log.w(TAG, "owner Binder died before connection: ${error.message}")
                breakBinding(this, OwnerClientTerminal.BinderDied)
                return
            }
            val messenger = Messenger(service)
            val queued = synchronized(lock) {
                when (val current = bindingState) {
                    is BindingState.Binding -> if (current.connection === this && current.id == bindingId) {
                        bindingState = BindingState.Connected(this, messenger, deathRecipient)
                        pending.values.toList()
                    } else {
                        emptyList()
                    }
                    BindingState.Unbound,
                    is BindingState.Connected,
                    BindingState.Closed -> emptyList()
                }
            }
            if (queued.isEmpty()) {
                val stillCurrent = synchronized(lock) {
                    (bindingState as? BindingState.Connected)?.connection === this
                }
                if (!stillCurrent) runCatching { service.unlinkToDeath(deathRecipient, 0) }
            } else {
                queued.forEach { dispatch(it, messenger) }
            }
        }

        override fun onServiceDisconnected(name: ComponentName?) =
            breakBinding(this, OwnerClientTerminal.BinderDied)

        override fun onBindingDied(name: ComponentName?) =
            breakBinding(this, OwnerClientTerminal.BindingDied)

        override fun onNullBinding(name: ComponentName?) =
            breakBinding(this, OwnerClientTerminal.NullBinding)
    }

    private fun dispatch(pendingRequest: Pending<*>, messenger: Messenger) {
        val sendResult = synchronized(lock) {
            if (pending[pendingRequest.requestId] !== pendingRequest) return
            when (pendingRequest.dispatchState) {
                PendingDispatchState.WaitingForBinding -> {
                    if (pendingRequest.isExpired()) {
                        pendingRequest.markCanceledBeforeDispatch()
                        DispatchResult.Expired
                    } else {
                        when (pendingLedger.markSent(pendingRequest.requestId)) {
                            OwnerIpcPendingSendAdmission.Sent -> {
                                pendingRequest.markSent(messenger)
                                pendingRequest.message.replyTo = replyMessenger
                                try {
                                    messenger.send(pendingRequest.message)
                                    DispatchResult.Sent
                                } catch (error: RemoteException) {
                                    Log.w(TAG, "owner IPC request send failed: ${error.message}")
                                    DispatchResult.SendFailed
                                }
                            }
                            OwnerIpcPendingSendAdmission.AlreadySent,
                            OwnerIpcPendingSendAdmission.CancellationPending,
                            OwnerIpcPendingSendAdmission.Unknown -> {
                                pendingRequest.markCanceledBeforeDispatch()
                                DispatchResult.AlreadyDispatched
                            }
                        }
                    }
                }
                is PendingDispatchState.Sent,
                PendingDispatchState.CanceledBeforeDispatch,
                PendingDispatchState.CancellationSent,
                PendingDispatchState.Terminal -> DispatchResult.AlreadyDispatched
            }
        }
        when (sendResult) {
            DispatchResult.Expired -> pendingRequest.fail(OwnerClientTerminal.ReplyTimedOut)
            DispatchResult.SendFailed -> breakBindingForCurrent(OwnerClientTerminal.SendFailed)
            DispatchResult.Sent,
            DispatchResult.AlreadyDispatched -> Unit
        }
    }

    private fun handleReply(message: Message): Boolean {
        val requestId = runCatching {
            message.data.getLong(MihomoControlService.REQUEST_ID, Long.MIN_VALUE)
        }.getOrDefault(Long.MIN_VALUE)
        val pendingRequest = synchronized(lock) { pending[requestId] }
        pendingRequest?.receive(message)
        return true
    }

    private fun breakBindingForCurrent(terminal: OwnerClientTerminal) {
        val connection = synchronized(lock) {
            when (val current = bindingState) {
                is BindingState.Binding -> current.connection
                is BindingState.Connected -> current.connection
                BindingState.Unbound,
                BindingState.Closed -> null
            }
        } ?: return
        breakBinding(connection, terminal)
    }

    private fun breakBinding(connection: ServiceConnection, terminal: OwnerClientTerminal) {
        val snapshot = synchronized(lock) {
            val current = bindingState
            val matches = when (current) {
                BindingState.Unbound,
                BindingState.Closed -> false
                is BindingState.Binding -> current.connection === connection
                is BindingState.Connected -> current.connection === connection
            }
            if (!matches) return
            val connected = current as? BindingState.Connected
            bindingState = BindingState.Unbound
            val requests = pending.values.toList()
            pending.clear()
            pendingLedger.disconnect(terminal)
            CloseSnapshot(connection, connected?.messenger, connected?.deathRecipient, requests)
        }
        snapshot.messenger?.binder?.let { binder ->
            snapshot.deathRecipient?.let { recipient -> runCatching { binder.unlinkToDeath(recipient, 0) } }
        }
        runCatching { applicationContext.unbindService(connection) }
        snapshot.pendingRequests.forEach { it.fail(terminal) }
    }

    private fun remove(pendingRequest: Pending<*>) {
        synchronized(lock) {
            if (pending[pendingRequest.requestId] === pendingRequest) {
                pending.remove(pendingRequest.requestId)
            }
            pendingLedger.complete(pendingRequest.requestId)
        }
    }

    private inner class Pending<T>(
        val request: AndroidHostRequest,
        val requestId: Long,
        private val deadline: VpnOwnerRequestDeadline,
        val message: Message,
        private val decodeReply: (Message) -> T,
        private val shouldAcknowledge: (T) -> Boolean,
        private val onFailure: (OwnerClientTerminal) -> T,
        private val onCancel: (Messenger) -> Unit,
        private val onAcknowledge: (Messenger) -> Unit,
    ) {
        private val latch = CountDownLatch(1)
        private val outcome = AtomicReference<PendingOutcome<T>>(PendingOutcome.Waiting)
        var dispatchState: PendingDispatchState = PendingDispatchState.WaitingForBinding
            private set

        fun isExpired(): Boolean =
            deadline.remainingAt(SystemClock.elapsedRealtime()) == VpnDeadlineState.Expired

        fun markSent(messenger: Messenger) {
            dispatchState = PendingDispatchState.Sent(messenger)
        }

        fun markCanceledBeforeDispatch() {
            dispatchState = PendingDispatchState.CanceledBeforeDispatch
        }

        fun receive(message: Message) {
            if (outcome.get() !is PendingOutcome.Waiting) return
            if (message.what != MihomoControlService.RESPONSE) {
                fail(OwnerClientTerminal.InvalidReply)
                return
            }
            val receivedRequestId = runCatching {
                message.data.getLong(MihomoControlService.REQUEST_ID, Long.MIN_VALUE)
            }.getOrDefault(Long.MIN_VALUE)
            if (receivedRequestId != requestId) {
                fail(OwnerClientTerminal.OwnerSessionStale)
                return
            }
            if (isExpired()) {
                cancel(OwnerClientTerminal.ReplyTimedOut)
                return
            }
            val messenger = synchronized(lock) {
                (dispatchState as? PendingDispatchState.Sent)?.messenger
            }
            val decoded = runCatching { decodeReply(message) }.getOrElse { error ->
                Log.w(TAG, "owner IPC reply decode failed: ${error.message}")
                fail(OwnerClientTerminal.InvalidReply)
                return
            }
            if (request == AndroidHostRequest.START_VPN_SERVICE && shouldAcknowledge(decoded)) {
                messenger?.let(onAcknowledge)
            }
            complete(decoded)
        }

        fun fail(terminal: OwnerClientTerminal) = complete(onFailure(terminal))

        fun awaitResult(): OwnerIpcTransportResult<T> {
            try {
                val remainingMillis = when (
                    val budget = deadline.remainingAt(SystemClock.elapsedRealtime())
                ) {
                    VpnDeadlineState.Expired -> 0L
                    is VpnDeadlineState.Remaining -> budget.millis
                }
                if (remainingMillis == 0L || !latch.await(remainingMillis, TimeUnit.MILLISECONDS)) {
                    cancel(OwnerClientTerminal.ReplyTimedOut)
                }
            } catch (error: InterruptedException) {
                Thread.currentThread().interrupt()
                cancel(OwnerClientTerminal.Interrupted)
            } finally {
                remove(this)
            }
            return when (val result = outcome.get()) {
                is PendingOutcome.Completed -> OwnerIpcTransportResult.Completed(result.response)
                PendingOutcome.Waiting -> OwnerIpcTransportResult.Completed(
                    onFailure(OwnerClientTerminal.OwnerUnavailable),
                )
            }
        }

        private fun cancel(terminal: OwnerClientTerminal) {
            val messenger = synchronized(lock) {
                when (val state = dispatchState) {
                    PendingDispatchState.WaitingForBinding -> {
                        when (pendingLedger.cancel(requestId)) {
                            OwnerIpcPendingCancellation.CanceledBeforeDispatch -> {
                                dispatchState = PendingDispatchState.CanceledBeforeDispatch
                                null
                            }
                            OwnerIpcPendingCancellation.CancelSentRequest,
                            OwnerIpcPendingCancellation.AlreadyCanceled,
                            OwnerIpcPendingCancellation.Unknown -> null
                        }
                    }
                    is PendingDispatchState.Sent -> {
                        when (pendingLedger.cancel(requestId)) {
                            OwnerIpcPendingCancellation.CancelSentRequest -> {
                                dispatchState = PendingDispatchState.CancellationSent
                                state.messenger
                            }
                            OwnerIpcPendingCancellation.CanceledBeforeDispatch,
                            OwnerIpcPendingCancellation.AlreadyCanceled,
                            OwnerIpcPendingCancellation.Unknown -> null
                        }
                    }
                    PendingDispatchState.CanceledBeforeDispatch,
                    PendingDispatchState.CancellationSent,
                    PendingDispatchState.Terminal -> null
                }
            }
            if (messenger != null) onCancel(messenger)
            fail(terminal)
        }

        private fun complete(response: T) {
            synchronized(lock) {
                if (outcome.compareAndSet(PendingOutcome.Waiting, PendingOutcome.Completed(response))) {
                    dispatchState = PendingDispatchState.Terminal
                    if (pending[requestId] === this) pending.remove(requestId)
                    pendingLedger.complete(requestId)
                    latch.countDown()
                }
            }
        }
    }

    private sealed interface BindingState {
        data object Unbound : BindingState

        data class Binding(val id: Long, val connection: ServiceConnection) : BindingState

        data class Connected(
            val connection: ServiceConnection,
            val messenger: Messenger,
            val deathRecipient: IBinder.DeathRecipient,
        ) : BindingState

        data object Closed : BindingState
    }

    private sealed interface PendingDispatchState {
        data object WaitingForBinding : PendingDispatchState

        data class Sent(val messenger: Messenger) : PendingDispatchState

        data object CanceledBeforeDispatch : PendingDispatchState

        data object CancellationSent : PendingDispatchState

        data object Terminal : PendingDispatchState
    }

    private sealed interface PendingOutcome<out T> {
        data object Waiting : PendingOutcome<Nothing>

        data class Completed<T>(val response: T) : PendingOutcome<T>
    }

    private sealed interface DispatchResult {
        data object Sent : DispatchResult

        data object Expired : DispatchResult

        data object SendFailed : DispatchResult

        data object AlreadyDispatched : DispatchResult
    }

    private enum class PendingAdmission {
        Added,
        Full,
        Duplicate,
        Closed,
    }

    private data class CloseSnapshot(
        val connection: ServiceConnection?,
        val messenger: Messenger?,
        val deathRecipient: IBinder.DeathRecipient?,
        val pendingRequests: List<Pending<*>>,
    )

    private companion object {
        const val TAG = "MihomoOwnerIpcConnection"
        const val MAX_PENDING_REQUESTS = 32
    }
}

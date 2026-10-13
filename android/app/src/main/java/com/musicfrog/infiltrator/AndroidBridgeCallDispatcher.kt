package com.musicfrog.infiltrator

import java.util.concurrent.ArrayBlockingQueue
import java.util.concurrent.FutureTask
import java.util.concurrent.RejectedExecutionException
import java.util.concurrent.ThreadFactory
import java.util.concurrent.ThreadPoolExecutor
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicInteger

internal enum class AndroidBridgeDispatchState(val wireCode: Int) {
    Accepted(0),

    QueueFull(1),

    DuplicateRequestId(2),

    HostUnavailable(3),

    InvalidRequest(4),
}

internal enum class AndroidBridgeCallCancellation(val wireCode: Int) {
    Canceled(0),
    AlreadyCompleted(1),
    UnknownRequest(2),
}

/** Application-owned bounded executor for Rust-to-Kotlin host calls. */
internal class AndroidBridgeCallDispatcher(
    workerCount: Int = DEFAULT_WORKER_COUNT,
    queueCapacity: Int = DEFAULT_QUEUE_CAPACITY,
) : AutoCloseable {
    private val lock = Any()
    private val requestStates = linkedMapOf<Long, RequestState>()
    private val maximumPendingCount = workerCount + queueCapacity
    private var dispatcherState: DispatcherState = DispatcherState.Open
    private val executor = ThreadPoolExecutor(
        workerCount,
        workerCount,
        0L,
        TimeUnit.MILLISECONDS,
        ArrayBlockingQueue(queueCapacity),
        NamedThreadFactory(),
        ThreadPoolExecutor.AbortPolicy(),
    )

    init {
        require(workerCount > 0)
        require(queueCapacity > 0)
    }

    fun submit(
        requestId: Long,
        call: AndroidBridgeCall,
        execute: (AndroidBridgeCall) -> AndroidBridgeCallReply,
        complete: (Long, AndroidBridgeCallReply) -> Unit,
        fail: (Long, String) -> Unit,
    ): AndroidBridgeDispatchState {
        if (requestId <= 0L) return AndroidBridgeDispatchState.HostUnavailable
        val task: FutureTask<Unit>
        synchronized(lock) {
            when (dispatcherState) {
                DispatcherState.Closed -> return AndroidBridgeDispatchState.HostUnavailable
                DispatcherState.Open -> Unit
            }
            if (requestStates.containsKey(requestId)) {
                return AndroidBridgeDispatchState.DuplicateRequestId
            }
            if (requestStates.size >= maximumPendingCount) {
                return AndroidBridgeDispatchState.QueueFull
            }
            task = FutureTask {
                if (!markRunning(requestId)) return@FutureTask Unit
                val reply = try {
                    execute(call)
                } catch (error: InterruptedException) {
                    Thread.currentThread().interrupt()
                    finishCanceled(requestId)
                    return@FutureTask Unit
                } catch (error: Exception) {
                    finishWithFailure(requestId, error.message ?: "Android host call failed", fail)
                    return@FutureTask Unit
                }
                if (finish(requestId)) complete(requestId, reply)
            }
            requestStates[requestId] = RequestState.Queued(task)
            try {
                executor.execute(task)
            } catch (_: RejectedExecutionException) {
                requestStates.remove(requestId)
                return AndroidBridgeDispatchState.QueueFull
            }
        }
        return AndroidBridgeDispatchState.Accepted
    }

    fun cancel(requestId: Long): AndroidBridgeCallCancellation {
        val task = synchronized(lock) {
            when (val state = requestStates.remove(requestId)) {
                null -> return AndroidBridgeCallCancellation.UnknownRequest
                is RequestState.Queued -> {
                    executor.remove(state.task)
                    state.task.cancel(false)
                    return AndroidBridgeCallCancellation.Canceled
                }
                is RequestState.Running -> state.task
            }
        }
        return when (task.cancel(true)) {
            true -> AndroidBridgeCallCancellation.Canceled
            false -> AndroidBridgeCallCancellation.AlreadyCompleted
        }
    }

    override fun close() {
        val tasks = synchronized(lock) {
            if (dispatcherState == DispatcherState.Closed) return
            dispatcherState = DispatcherState.Closed
            requestStates.values.map(RequestState::task).also { requestStates.clear() }
        }
        tasks.forEach { task -> task.cancel(true) }
        executor.shutdownNow()
    }

    private fun markRunning(requestId: Long): Boolean = synchronized(lock) {
        when (val state = requestStates[requestId]) {
            is RequestState.Queued -> {
                requestStates[requestId] = RequestState.Running(state.task)
                true
            }
            is RequestState.Running,
            null -> false
        }
    }

    private fun finish(requestId: Long): Boolean = synchronized(lock) {
        when (requestStates[requestId]) {
            is RequestState.Running -> {
                requestStates.remove(requestId)
                true
            }
            is RequestState.Queued,
            null -> false
        }
    }

    private fun finishCanceled(requestId: Long) {
        synchronized(lock) {
            if (requestStates[requestId] is RequestState.Running) requestStates.remove(requestId)
        }
    }

    private fun finishWithFailure(
        requestId: Long,
        message: String,
        fail: (Long, String) -> Unit,
    ) {
        if (finish(requestId)) fail(requestId, message)
    }

    private sealed interface RequestState {
        val task: FutureTask<Unit>

        data class Queued(override val task: FutureTask<Unit>) : RequestState

        data class Running(override val task: FutureTask<Unit>) : RequestState
    }

    private enum class DispatcherState {
        Open,
        Closed,
    }

    private class NamedThreadFactory : ThreadFactory {
        private val sequence = AtomicInteger()

        override fun newThread(task: Runnable): Thread = Thread(
            task,
            "musicfrog-rust-host-${sequence.incrementAndGet()}",
        )
    }

    private companion object {
        const val DEFAULT_WORKER_COUNT = 4
        const val DEFAULT_QUEUE_CAPACITY = 16
    }
}

package com.musicfrog.infiltrator

import android.content.Context
import android.system.Os
import android.system.OsConstants
import android.os.SystemClock
import android.util.Log
import java.io.File
import java.io.IOException
import java.util.concurrent.TimeUnit

internal enum class CoreProcessStartReceipt(val wireCode: Int) {
    Started(0),
    AlreadyRunning(1),
    BinaryUnavailable(2),
    LaunchSpecRejected(3),
    SpawnFailed(4),
    NotVpnOwner(5),
    OwnerContextMismatch(6),
    PreviousChildExitPending(7),
    OwnershipRecordUnavailable(8);

    companion object {
        fun fromWireCode(code: Int): CoreProcessStartReceipt? = when (code) {
            0 -> Started
            1 -> AlreadyRunning
            2 -> BinaryUnavailable
            3 -> LaunchSpecRejected
            4 -> SpawnFailed
            5 -> NotVpnOwner
            6 -> OwnerContextMismatch
            7 -> PreviousChildExitPending
            8 -> OwnershipRecordUnavailable
            else -> null
        }
    }
}

internal enum class CoreProcessStopReceipt(val wireCode: Int) {
    StoppedGracefully(0),
    StoppedForcibly(1),
    AlreadyStopped(2),
    TimedOut(3),
    Interrupted(4),
    NotVpnOwner(5),
    OwnershipRecordUnavailable(6);

    companion object {
        fun fromWireCode(code: Int): CoreProcessStopReceipt? = when (code) {
            0 -> StoppedGracefully
            1 -> StoppedForcibly
            2 -> AlreadyStopped
            3 -> TimedOut
            4 -> Interrupted
            5 -> NotVpnOwner
            6 -> OwnershipRecordUnavailable
            else -> null
        }
    }
}

internal data class CoreProcessSpawnCommand(
    val arguments: List<String>,
    val workingDirectory: File,
) {
    fun processBuilder(): ProcessBuilder = ProcessBuilder(arguments)
        .directory(workingDirectory)
        .redirectErrorStream(true)

    companion object {
        fun from(binary: File, spec: CoreLaunchSpecWire): CoreProcessSpawnCommand {
            val engineArguments = when (val launch = spec.launchArguments) {
                is CoreLaunchArgumentsWire.Mihomo -> listOf(
                    "-d",
                    spec.workingDirectory,
                    "-f",
                    launch.configurationPath,
                )
                is CoreLaunchArgumentsWire.SingBox -> listOf(
                    "run",
                    "-c",
                    launch.configurationPath,
                )
            }
            return CoreProcessSpawnCommand(
                arguments = listOf(binary.absolutePath) + engineArguments,
                workingDirectory = File(spec.workingDirectory),
            )
        }
    }
}

internal fun interface CoreProcessStarter {
    fun start(builder: ProcessBuilder): Process
}

internal class CoreProcessController(
    private val nativeLibraryDirectory: File,
    private val filesDirectory: File,
    private val processStarter: CoreProcessStarter = CoreProcessStarter { it.start() },
    private val elapsedRealtimeMillis: () -> Long = { SystemClock.elapsedRealtime() },
    private val recordStore: AndroidCoreProcessRecordStore = AndroidCoreProcessRecordStore(filesDirectory),
    private val childInspector: AndroidChildProcessInspector = ProcAndroidChildProcessInspector(),
    private val childSignalPort: AndroidChildProcessSignalPort = AndroidChildProcessSignalPort { pid, signal ->
        val osSignal = when (signal) {
            AndroidChildProcessSignal.Graceful -> OsConstants.SIGTERM
            AndroidChildProcessSignal.Force -> OsConstants.SIGKILL
        }
        runCatching { Os.kill(pid, osSignal) }
            .fold(
                onSuccess = { AndroidChildProcessSignalOutcome.Sent },
                onFailure = { AndroidChildProcessSignalOutcome.Unavailable },
            )
    },
) {
    constructor(context: Context) : this(
        nativeLibraryDirectory = File(context.applicationInfo.nativeLibraryDir),
        filesDirectory = context.filesDir,
    )

    private sealed interface ProcessState {
        data object NeverStarted : ProcessState

        sealed interface Running : ProcessState {
            val owned: AndroidOwnedCoreProcessWire
        }

        data class RunningWithHandle(
            val process: Process,
            override val owned: AndroidOwnedCoreProcessWire,
        ) : Running

        data class RunningByIdentity(
            override val owned: AndroidOwnedCoreProcessWire,
        ) : Running

        data class Exited(
            val owned: AndroidOwnedCoreProcessWire,
            val evidence: AndroidCoreProcessExitEvidenceWire,
        ) : ProcessState

        data class Unavailable(
            val reason: AndroidCoreProcessUnavailableReasonWire,
        ) : ProcessState
    }

    private enum class ProcessWaitOutcome {
        Exited,
        TimedOut,
    }

    private enum class ProcessHandleObservation {
        Running,
        Exited,
    }

    private sealed interface ProcessSignalOutcome {
        data object Sent : ProcessSignalOutcome

        data class Exited(val evidence: AndroidCoreProcessExitEvidenceWire) : ProcessSignalOutcome

        data object Unavailable : ProcessSignalOutcome
    }

    private enum class OwnedProcessWaitOutcome {
        Exited,
        TimedOut,
        Unavailable,
        Interrupted,
    }

    @Volatile
    private var processState: ProcessState = restoreProcessState()
    private val binaryResolver = AndroidCoreBinaryResolver(nativeLibraryDirectory, filesDirectory)
    private val configurationValidator = MihomoCoreConfigurationValidator(
        nativeLibraryDirectory = nativeLibraryDirectory,
        filesDirectory = filesDirectory,
        processStarter = processStarter,
        elapsedRealtimeMillis = elapsedRealtimeMillis,
    )

    @Synchronized
    fun start(
        spec: CoreLaunchSpecWire,
        context: CoreProcessStartContextWire,
    ): CoreProcessStartReceipt {
        if (spec.configurationIdentity.engineId != context.identity.engine ||
            spec.configurationIdentity.engineId != spec.binary.engineId.wireValue ||
            spec.configurationIdentity.engineId != spec.launchArguments.engineId.wireValue
        ) {
            return CoreProcessStartReceipt.LaunchSpecRejected
        }
        return when (configurationValidator.coreStartReadiness()) {
            CoreValidationWorkerReadiness.CleanupPending -> CoreProcessStartReceipt.SpawnFailed
            CoreValidationWorkerReadiness.Ready -> startWithBinary(spec, context)
        }
    }

    private fun startWithBinary(
        spec: CoreLaunchSpecWire,
        context: CoreProcessStartContextWire,
    ): CoreProcessStartReceipt = when (val state = reconcileProcessState()) {
        is ProcessState.RunningWithHandle -> if (state.owned.context == context) {
            CoreProcessStartReceipt.AlreadyRunning
        } else {
            CoreProcessStartReceipt.OwnerContextMismatch
        }
        is ProcessState.RunningByIdentity -> if (state.owned.context == context) {
            CoreProcessStartReceipt.AlreadyRunning
        } else {
            CoreProcessStartReceipt.OwnerContextMismatch
        }
        is ProcessState.Exited -> CoreProcessStartReceipt.PreviousChildExitPending
        is ProcessState.Unavailable -> CoreProcessStartReceipt.OwnershipRecordUnavailable
        ProcessState.NeverStarted -> when (val binary = binaryResolver.resolve(spec.binary)) {
            CoreBinaryResolution.Missing -> CoreProcessStartReceipt.BinaryUnavailable
            is CoreBinaryResolution.Rejected -> {
                Log.w(TAG_PROCESS, "Core launch binary was rejected: ${binary.reason}")
                CoreProcessStartReceipt.LaunchSpecRejected
            }
            is CoreBinaryResolution.Available -> when (spec.binary) {
                CoreLaunchBinaryWire.MihomoBundled -> spawn(binary.file, spec, context)
                is CoreLaunchBinaryWire.SingBoxManaged -> spawn(binary.file, spec, context)
            }
        }
    }

    @Synchronized
    fun validateConfiguration(
        request: CoreConfigurationValidationRequestWire,
    ): CoreConfigurationValidationOutcomeWire = when (reconcileProcessState()) {
        is ProcessState.RunningWithHandle,
        is ProcessState.RunningByIdentity -> CoreConfigurationValidationOutcomeWire.CoreActive
        is ProcessState.Unavailable ->
            CoreConfigurationValidationOutcomeWire.CoreProcessObservationUnavailable
        ProcessState.NeverStarted,
        is ProcessState.Exited -> configurationValidator.validate(request)
    }

    private fun spawn(
        binary: File,
        spec: CoreLaunchSpecWire,
        context: CoreProcessStartContextWire,
    ): CoreProcessStartReceipt {
        val launchSnapshot = when (val snapshot = childInspector.snapshotCurrentThreadChildren()) {
            AndroidCurrentThreadChildSnapshot.Unavailable ->
                return CoreProcessStartReceipt.OwnershipRecordUnavailable
            is AndroidCurrentThreadChildSnapshot.Captured -> snapshot
        }
        when (recordStore.writeReservation(context)) {
            AndroidCoreProcessRecordMutationOutcome.Committed -> Unit
            AndroidCoreProcessRecordMutationOutcome.Unavailable -> {
                processState = ProcessState.Unavailable(
                    AndroidCoreProcessUnavailableReasonWire.OwnershipRecordUnavailable,
                )
                return CoreProcessStartReceipt.OwnershipRecordUnavailable
            }
        }
        val child = try {
        val command = CoreProcessSpawnCommand.from(binary, spec)
            processStarter.start(command.processBuilder())
        } catch (error: IOException) {
            Log.w(TAG_PROCESS, "Core spawn failed: ${error.message}")
            clearFailedReservation()
            return CoreProcessStartReceipt.SpawnFailed
        } catch (error: SecurityException) {
            Log.w(TAG_PROCESS, "Core spawn was denied: ${error.message}")
            clearFailedReservation()
            return CoreProcessStartReceipt.SpawnFailed
        }
        val childIdentity = when (
            val capture = childInspector.captureStartedChild(launchSnapshot)
        ) {
            is AndroidChildProcessCapture.Identified -> capture.child
            AndroidChildProcessCapture.Exited,
            AndroidChildProcessCapture.Unavailable -> {
                child.destroyForcibly()
                when (waitForExit(child, FORCE_STOP_GRACE_SECONDS)) {
                    ProcessWaitOutcome.Exited -> clearFailedReservation()
                    ProcessWaitOutcome.TimedOut -> {
                        processState = ProcessState.Unavailable(
                            AndroidCoreProcessUnavailableReasonWire.LaunchReservationIncomplete,
                        )
                    }
                }
                return CoreProcessStartReceipt.SpawnFailed
            }
        }
        val owned = AndroidOwnedCoreProcessWire(context, childIdentity)
        when (recordStore.writeOwned(owned)) {
            AndroidCoreProcessRecordMutationOutcome.Committed -> Unit
            AndroidCoreProcessRecordMutationOutcome.Unavailable -> {
                child.destroyForcibly()
                when (waitForExit(child, FORCE_STOP_GRACE_SECONDS)) {
                    ProcessWaitOutcome.Exited -> clearFailedReservation()
                    ProcessWaitOutcome.TimedOut -> {
                        processState = ProcessState.Unavailable(
                            AndroidCoreProcessUnavailableReasonWire.OwnershipRecordUnavailable,
                        )
                    }
                }
                return CoreProcessStartReceipt.OwnershipRecordUnavailable
            }
        }
        processState = ProcessState.RunningWithHandle(child, owned)
        Thread {
            try {
                child.inputStream.bufferedReader().use { reader ->
                    reader.forEachLine { line -> Log.d(TAG_CORE, line) }
                }
            } catch (error: Exception) {
                Log.w(TAG_PROCESS, "Core log reader failed: ${error.message}")
            }
        }.start()
        // Rust CoreReadiness owns the API probe and rolls back failed launches.
        return CoreProcessStartReceipt.Started
    }

    @Synchronized
    fun stop(): CoreProcessStopReceipt = when (val state = reconcileProcessState()) {
        ProcessState.NeverStarted -> CoreProcessStopReceipt.AlreadyStopped
        // Keep exact exit evidence until the Rust owner explicitly consumes
        // it during recovery; a generic stop call cannot prove that lease was
        // released by the old owner session.
        is ProcessState.Exited -> CoreProcessStopReceipt.AlreadyStopped
        is ProcessState.Unavailable -> CoreProcessStopReceipt.OwnershipRecordUnavailable
        is ProcessState.RunningWithHandle -> stop(state.process)
        is ProcessState.RunningByIdentity -> stopOwnedChild(state.owned)
    }

    private fun stop(current: Process): CoreProcessStopReceipt = try {
        current.destroy()
        when (waitForExit(current, STOP_GRACE_SECONDS)) {
            ProcessWaitOutcome.Exited -> clearStoppedRecord(CoreProcessStopReceipt.StoppedGracefully)
            ProcessWaitOutcome.TimedOut -> {
                current.destroyForcibly()
                when (waitForExit(current, FORCE_STOP_GRACE_SECONDS)) {
                    ProcessWaitOutcome.Exited -> clearStoppedRecord(CoreProcessStopReceipt.StoppedForcibly)
                    ProcessWaitOutcome.TimedOut -> {
                        Log.e(TAG_PROCESS, "Mihomo ignored graceful and forced stop deadlines")
                        CoreProcessStopReceipt.TimedOut
                    }
                }
            }
        }
    } catch (error: InterruptedException) {
        Log.w(TAG_PROCESS, "Core stop was interrupted: ${error.message}")
        Thread.currentThread().interrupt()
        CoreProcessStopReceipt.Interrupted
    }

    @Synchronized
    fun observation(): AndroidCoreProcessObservationWire = when (val state = reconcileProcessState()) {
        ProcessState.NeverStarted -> AndroidCoreProcessObservationWire.NeverStarted
        is ProcessState.RunningWithHandle -> AndroidCoreProcessObservationWire.Running(state.owned)
        is ProcessState.RunningByIdentity -> AndroidCoreProcessObservationWire.Running(state.owned)
        is ProcessState.Exited -> AndroidCoreProcessObservationWire.Exited(state.owned, state.evidence)
        is ProcessState.Unavailable -> AndroidCoreProcessObservationWire.Unavailable(state.reason)
    }

    @Synchronized
    fun consumeRecoveredExit(
        context: CoreProcessStartContextWire,
    ): AndroidCoreProcessRecoveryReceiptWire = when (val state = reconcileProcessState()) {
        ProcessState.NeverStarted -> AndroidCoreProcessRecoveryReceiptWire.AlreadyAbsent
        is ProcessState.RunningWithHandle -> when (state.owned.context == context) {
            true -> AndroidCoreProcessRecoveryReceiptWire.ChildStillRunning
            false -> AndroidCoreProcessRecoveryReceiptWire.ContextMismatch
        }
        is ProcessState.RunningByIdentity -> when (state.owned.context == context) {
            true -> AndroidCoreProcessRecoveryReceiptWire.ChildStillRunning
            false -> AndroidCoreProcessRecoveryReceiptWire.ContextMismatch
        }
        is ProcessState.Exited -> when (state.owned.context == context) {
            false -> AndroidCoreProcessRecoveryReceiptWire.ContextMismatch
            true -> when (val stored = recordStore.read()) {
                StoredAndroidCoreProcessRecord.Missing -> {
                    processState = ProcessState.NeverStarted
                    AndroidCoreProcessRecoveryReceiptWire.AlreadyAbsent
                }
                is StoredAndroidCoreProcessRecord.LaunchReserved,
                StoredAndroidCoreProcessRecord.Corrupt ->
                    AndroidCoreProcessRecoveryReceiptWire.ObservationUnavailable
                is StoredAndroidCoreProcessRecord.Owned -> when (stored.process) {
                    state.owned -> when (recordStore.clear()) {
                        AndroidCoreProcessRecordMutationOutcome.Committed -> {
                            processState = ProcessState.NeverStarted
                            AndroidCoreProcessRecoveryReceiptWire.Consumed
                        }
                        AndroidCoreProcessRecordMutationOutcome.Unavailable -> {
                            processState = ProcessState.Unavailable(
                                AndroidCoreProcessUnavailableReasonWire.OwnershipRecordUnavailable,
                            )
                            AndroidCoreProcessRecoveryReceiptWire.ObservationUnavailable
                        }
                    }
                    else -> AndroidCoreProcessRecoveryReceiptWire.ContextMismatch
                }
            }
        }
        is ProcessState.Unavailable -> AndroidCoreProcessRecoveryReceiptWire.ObservationUnavailable
    }

    private fun reconcileProcessState(): ProcessState = when (val current = processState) {
        ProcessState.NeverStarted,
        is ProcessState.Exited,
        is ProcessState.Unavailable -> current
        is ProcessState.RunningWithHandle -> when (processHandleObservation(current.process)) {
            ProcessHandleObservation.Exited -> ProcessState.Exited(
                current.owned,
                AndroidCoreProcessExitEvidenceWire.ProcessHandleExited,
            ).also { processState = it }
            ProcessHandleObservation.Running -> reconcileOwnedProcess(current.owned, current)
        }
        is ProcessState.RunningByIdentity -> reconcileOwnedProcess(current.owned, current)
    }

    private fun processHandleObservation(process: Process): ProcessHandleObservation =
        if (process.isAlive) ProcessHandleObservation.Running else ProcessHandleObservation.Exited

    private fun reconcileOwnedProcess(
        owned: AndroidOwnedCoreProcessWire,
        running: ProcessState.Running,
    ): ProcessState = when (val inspection = childInspector.inspect(owned.child)) {
        AndroidChildProcessInspection.ExactChildRunning -> running
        is AndroidChildProcessInspection.Exited -> ProcessState.Exited(owned, inspection.evidence)
            .also { processState = it }
        AndroidChildProcessInspection.Unavailable -> ProcessState.Unavailable(
            AndroidCoreProcessUnavailableReasonWire.ProcessInspectionUnavailable,
        ).also { processState = it }
    }

    private fun restoreProcessState(): ProcessState = when (val record = recordStore.read()) {
        StoredAndroidCoreProcessRecord.Missing -> ProcessState.NeverStarted
        is StoredAndroidCoreProcessRecord.LaunchReserved -> ProcessState.Unavailable(
            AndroidCoreProcessUnavailableReasonWire.LaunchReservationIncomplete,
        )
        StoredAndroidCoreProcessRecord.Corrupt -> ProcessState.Unavailable(
            AndroidCoreProcessUnavailableReasonWire.OwnershipRecordCorrupt,
        )
        is StoredAndroidCoreProcessRecord.Owned -> when (
            val inspection = childInspector.inspect(record.process.child)
        ) {
            AndroidChildProcessInspection.ExactChildRunning -> ProcessState.RunningByIdentity(record.process)
            is AndroidChildProcessInspection.Exited ->
                ProcessState.Exited(record.process, inspection.evidence)
            AndroidChildProcessInspection.Unavailable -> ProcessState.Unavailable(
                AndroidCoreProcessUnavailableReasonWire.ProcessInspectionUnavailable,
            )
        }
    }

    private fun clearFailedReservation() {
        processState = when (recordStore.clear()) {
            AndroidCoreProcessRecordMutationOutcome.Committed -> ProcessState.NeverStarted
            AndroidCoreProcessRecordMutationOutcome.Unavailable -> ProcessState.Unavailable(
                AndroidCoreProcessUnavailableReasonWire.OwnershipRecordUnavailable,
            )
        }
    }

    private fun clearStoppedRecord(receipt: CoreProcessStopReceipt): CoreProcessStopReceipt =
        when (recordStore.clear()) {
            AndroidCoreProcessRecordMutationOutcome.Committed -> {
                processState = ProcessState.NeverStarted
                receipt
            }
            AndroidCoreProcessRecordMutationOutcome.Unavailable -> {
                processState = ProcessState.Unavailable(
                    AndroidCoreProcessUnavailableReasonWire.OwnershipRecordUnavailable,
                )
                CoreProcessStopReceipt.OwnershipRecordUnavailable
            }
        }

    private fun stopOwnedChild(owned: AndroidOwnedCoreProcessWire): CoreProcessStopReceipt =
        when (val inspection = childInspector.inspect(owned.child)) {
            is AndroidChildProcessInspection.Exited -> recordOwnedExit(
                owned,
                inspection.evidence,
                CoreProcessStopReceipt.AlreadyStopped,
            )
            AndroidChildProcessInspection.Unavailable -> CoreProcessStopReceipt.OwnershipRecordUnavailable
            AndroidChildProcessInspection.ExactChildRunning -> when (
                val signal = signalExactChild(owned, AndroidChildProcessSignal.Graceful)
            ) {
                ProcessSignalOutcome.Sent -> awaitOwnedStop(owned)
                is ProcessSignalOutcome.Exited -> recordOwnedExit(
                    owned,
                    signal.evidence,
                    CoreProcessStopReceipt.AlreadyStopped,
                )
                ProcessSignalOutcome.Unavailable -> CoreProcessStopReceipt.OwnershipRecordUnavailable
            }
        }

    private fun awaitOwnedStop(owned: AndroidOwnedCoreProcessWire): CoreProcessStopReceipt =
        when (waitForOwnedExit(owned, STOP_GRACE_SECONDS)) {
            OwnedProcessWaitOutcome.Exited -> recordOwnedExit(
                owned,
                AndroidCoreProcessExitEvidenceWire.ProcessHandleExited,
                CoreProcessStopReceipt.StoppedGracefully,
            )
            OwnedProcessWaitOutcome.Unavailable -> CoreProcessStopReceipt.OwnershipRecordUnavailable
            OwnedProcessWaitOutcome.Interrupted -> CoreProcessStopReceipt.Interrupted
            OwnedProcessWaitOutcome.TimedOut -> when (
                val signal = signalExactChild(owned, AndroidChildProcessSignal.Force)
            ) {
                ProcessSignalOutcome.Sent -> when (waitForOwnedExit(owned, FORCE_STOP_GRACE_SECONDS)) {
                    OwnedProcessWaitOutcome.Exited -> recordOwnedExit(
                        owned,
                        AndroidCoreProcessExitEvidenceWire.ProcessHandleExited,
                        CoreProcessStopReceipt.StoppedForcibly,
                    )
                    OwnedProcessWaitOutcome.TimedOut -> CoreProcessStopReceipt.TimedOut
                    OwnedProcessWaitOutcome.Unavailable -> CoreProcessStopReceipt.OwnershipRecordUnavailable
                    OwnedProcessWaitOutcome.Interrupted -> CoreProcessStopReceipt.Interrupted
                }
                is ProcessSignalOutcome.Exited -> recordOwnedExit(
                    owned,
                    signal.evidence,
                    CoreProcessStopReceipt.AlreadyStopped,
                )
                ProcessSignalOutcome.Unavailable -> CoreProcessStopReceipt.OwnershipRecordUnavailable
            }
        }

    private fun recordOwnedExit(
        owned: AndroidOwnedCoreProcessWire,
        evidence: AndroidCoreProcessExitEvidenceWire,
        receipt: CoreProcessStopReceipt,
    ): CoreProcessStopReceipt {
        processState = ProcessState.Exited(owned, evidence)
        return receipt
    }

    private fun signalExactChild(
        owned: AndroidOwnedCoreProcessWire,
        signal: AndroidChildProcessSignal,
    ): ProcessSignalOutcome =
        when (val inspection = childInspector.inspect(owned.child)) {
            AndroidChildProcessInspection.ExactChildRunning -> when (
                childSignalPort.signal(owned.child.pid.toInt(), signal)
            ) {
                AndroidChildProcessSignalOutcome.Sent -> ProcessSignalOutcome.Sent
                AndroidChildProcessSignalOutcome.Unavailable -> ProcessSignalOutcome.Unavailable
            }
            is AndroidChildProcessInspection.Exited -> ProcessSignalOutcome.Exited(inspection.evidence)
            AndroidChildProcessInspection.Unavailable -> ProcessSignalOutcome.Unavailable
        }

    private fun waitForOwnedExit(
        owned: AndroidOwnedCoreProcessWire,
        timeoutSeconds: Long,
    ): OwnedProcessWaitOutcome {
        val deadline = elapsedRealtimeMillis() + TimeUnit.SECONDS.toMillis(timeoutSeconds)
        try {
            while (elapsedRealtimeMillis() < deadline) {
                when (childInspector.inspect(owned.child)) {
                    is AndroidChildProcessInspection.Exited -> return OwnedProcessWaitOutcome.Exited
                    AndroidChildProcessInspection.Unavailable -> return OwnedProcessWaitOutcome.Unavailable
                    AndroidChildProcessInspection.ExactChildRunning -> Thread.sleep(POLL_INTERVAL_MILLIS)
                }
            }
        } catch (error: InterruptedException) {
            Thread.currentThread().interrupt()
            return OwnedProcessWaitOutcome.Interrupted
        }
        return when (childInspector.inspect(owned.child)) {
            is AndroidChildProcessInspection.Exited -> OwnedProcessWaitOutcome.Exited
            AndroidChildProcessInspection.Unavailable -> OwnedProcessWaitOutcome.Unavailable
            AndroidChildProcessInspection.ExactChildRunning -> OwnedProcessWaitOutcome.TimedOut
        }
    }

    private fun waitForExit(current: Process, timeoutSeconds: Long): ProcessWaitOutcome =
        when (current.waitFor(timeoutSeconds, TimeUnit.SECONDS)) {
            true -> ProcessWaitOutcome.Exited
            false -> ProcessWaitOutcome.TimedOut
        }

    private companion object {
        const val TAG_PROCESS = "CoreProcess"
        const val TAG_CORE = "CoreProcessOutput"
        const val STOP_GRACE_SECONDS = 3L
        const val FORCE_STOP_GRACE_SECONDS = 2L
        const val POLL_INTERVAL_MILLIS = 50L
    }
}

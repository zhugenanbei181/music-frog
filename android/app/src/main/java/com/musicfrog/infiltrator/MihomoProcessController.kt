package com.musicfrog.infiltrator

import android.content.Context
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
    NotVpnOwner(5);

    companion object {
        fun fromWireCode(code: Int): CoreProcessStartReceipt? = when (code) {
            0 -> Started
            1 -> AlreadyRunning
            2 -> BinaryUnavailable
            3 -> LaunchSpecRejected
            4 -> SpawnFailed
            5 -> NotVpnOwner
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
    NotVpnOwner(5);

    companion object {
        fun fromWireCode(code: Int): CoreProcessStopReceipt? = when (code) {
            0 -> StoppedGracefully
            1 -> StoppedForcibly
            2 -> AlreadyStopped
            3 -> TimedOut
            4 -> Interrupted
            5 -> NotVpnOwner
            else -> null
        }
    }
}

internal data class MihomoSpawnCommand(
    val arguments: List<String>,
    val workingDirectory: File,
) {
    fun processBuilder(): ProcessBuilder = ProcessBuilder(arguments)
        .directory(workingDirectory)
        .redirectErrorStream(true)

    companion object {
        fun from(binary: File, spec: CoreLaunchSpecWire): MihomoSpawnCommand = MihomoSpawnCommand(
            arguments = listOf(binary.absolutePath) + spec.launchArguments,
            workingDirectory = File(spec.workingDirectory),
        )
    }
}

internal fun interface MihomoProcessStarter {
    fun start(builder: ProcessBuilder): Process
}

internal class MihomoProcessController(
    private val nativeLibraryDirectory: File,
    private val filesDirectory: File,
    private val processStarter: MihomoProcessStarter = MihomoProcessStarter { it.start() },
    private val elapsedRealtimeMillis: () -> Long = { SystemClock.elapsedRealtime() },
) {
    constructor(context: Context) : this(
        nativeLibraryDirectory = File(context.applicationInfo.nativeLibraryDir),
        filesDirectory = context.filesDir,
    )

    private sealed interface ProcessState {
        data object Stopped : ProcessState

        data class Running(val process: Process) : ProcessState
    }

    private sealed interface CoreBinaryResolution {
        data class Available(val file: File) : CoreBinaryResolution

        data object Missing : CoreBinaryResolution
    }

    private enum class ProcessWaitOutcome {
        Exited,
        TimedOut,
    }

    @Volatile
    private var processState: ProcessState = ProcessState.Stopped
    private val configurationValidator = MihomoCoreConfigurationValidator(
        nativeLibraryDirectory = nativeLibraryDirectory,
        filesDirectory = filesDirectory,
        processStarter = processStarter,
        elapsedRealtimeMillis = elapsedRealtimeMillis,
    )

    @Synchronized
    fun start(spec: CoreLaunchSpecWire): CoreProcessStartReceipt =
        when (configurationValidator.coreStartReadiness()) {
            CoreValidationWorkerReadiness.CleanupPending -> CoreProcessStartReceipt.SpawnFailed
            CoreValidationWorkerReadiness.Ready -> when (reconcileProcessState()) {
                is ProcessState.Running -> CoreProcessStartReceipt.AlreadyRunning
                ProcessState.Stopped -> when (val binary = resolveBinary()) {
                    CoreBinaryResolution.Missing -> CoreProcessStartReceipt.BinaryUnavailable
                    is CoreBinaryResolution.Available -> spawn(binary.file, spec)
                }
            }
        }

    @Synchronized
    fun validateConfiguration(
        request: CoreConfigurationValidationRequestWire,
    ): CoreConfigurationValidationOutcomeWire = when (reconcileProcessState()) {
        is ProcessState.Running -> CoreConfigurationValidationOutcomeWire.CoreActive
        ProcessState.Stopped -> configurationValidator.validate(request)
    }

    private fun spawn(binary: File, spec: CoreLaunchSpecWire): CoreProcessStartReceipt = try {
        val command = MihomoSpawnCommand.from(binary, spec)
        val child = processStarter.start(command.processBuilder())
        processState = ProcessState.Running(child)
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
        CoreProcessStartReceipt.Started
    } catch (error: IOException) {
        Log.w(TAG_PROCESS, "Core spawn failed: ${error.message}")
        CoreProcessStartReceipt.SpawnFailed
    } catch (error: SecurityException) {
        Log.w(TAG_PROCESS, "Core spawn was denied: ${error.message}")
        CoreProcessStartReceipt.SpawnFailed
    }

    @Synchronized
    fun stop(): CoreProcessStopReceipt = when (val state = reconcileProcessState()) {
        ProcessState.Stopped -> CoreProcessStopReceipt.AlreadyStopped
        is ProcessState.Running -> stop(state.process)
    }

    private fun stop(current: Process): CoreProcessStopReceipt = try {
        current.destroy()
        when (waitForExit(current, STOP_GRACE_SECONDS)) {
            ProcessWaitOutcome.Exited -> {
                processState = ProcessState.Stopped
                CoreProcessStopReceipt.StoppedGracefully
            }
            ProcessWaitOutcome.TimedOut -> {
                current.destroyForcibly()
                when (waitForExit(current, FORCE_STOP_GRACE_SECONDS)) {
                    ProcessWaitOutcome.Exited -> {
                        processState = ProcessState.Stopped
                        CoreProcessStopReceipt.StoppedForcibly
                    }
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
    fun state(): AndroidCoreProcessState = when (reconcileProcessState()) {
        ProcessState.Stopped -> AndroidCoreProcessState.STOPPED
        is ProcessState.Running -> AndroidCoreProcessState.RUNNING
    }

    private fun reconcileProcessState(): ProcessState = when (val current = processState) {
        ProcessState.Stopped -> ProcessState.Stopped
        is ProcessState.Running -> if (current.process.isAlive) {
            current
        } else {
            processState = ProcessState.Stopped
            ProcessState.Stopped
        }
    }

    private fun resolveBinary(): CoreBinaryResolution {
        // Android 10+ requires executable binaries in nativeLibraryDir.
        val binary = File(nativeLibraryDirectory, BINARY_NAME)
        return if (binary.exists()) CoreBinaryResolution.Available(binary) else {
            Log.w(TAG_PROCESS, "Mihomo binary is missing at ${binary.absolutePath}")
            CoreBinaryResolution.Missing
        }
    }

    private fun waitForExit(current: Process, timeoutSeconds: Long): ProcessWaitOutcome =
        when (current.waitFor(timeoutSeconds, TimeUnit.SECONDS)) {
            true -> ProcessWaitOutcome.Exited
            false -> ProcessWaitOutcome.TimedOut
        }

    private companion object {
        const val BINARY_NAME = "libmihomo.so"
        const val TAG_PROCESS = "MihomoProcess"
        const val TAG_CORE = "MihomoCore"
        const val STOP_GRACE_SECONDS = 3L
        const val FORCE_STOP_GRACE_SECONDS = 2L
    }
}

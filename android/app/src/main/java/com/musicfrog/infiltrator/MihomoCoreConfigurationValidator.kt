package com.musicfrog.infiltrator

import java.io.File
import java.io.IOException
import java.security.MessageDigest
import java.util.concurrent.TimeUnit

internal class MihomoCoreConfigurationValidator(
    private val nativeLibraryDirectory: File,
    private val filesDirectory: File,
    private val processStarter: MihomoProcessStarter,
    private val elapsedRealtimeMillis: () -> Long,
) {
    private sealed interface ValidationWorkerState {
        data object Ready : ValidationWorkerState
        data class CleanupPending(val child: Process) : ValidationWorkerState
    }

    private var workerState: ValidationWorkerState = ValidationWorkerState.Ready

    private sealed interface PathValidation {
        data class Accepted(
            val binary: File,
            val candidate: File,
            val workingDirectory: File,
        ) : PathValidation

        data object BinaryUnavailable : PathValidation
        data object CandidateMissing : PathValidation
        data object PathRejected : PathValidation
        data object DigestMismatch : PathValidation
        data object IdentityDigestMismatch : PathValidation
    }

    private sealed interface ProcessWaitOutcome {
        data object Exited : ProcessWaitOutcome
        data object TimedOut : ProcessWaitOutcome
    }

    private sealed interface ProcessCleanupOutcome {
        data object Reaped : ProcessCleanupOutcome
        data object StillRunning : ProcessCleanupOutcome
    }

    private sealed interface ChildExecutionOutcome {
        data class Exited(
            val exitCode: Int,
            val digestMatches: Boolean,
        ) : ChildExecutionOutcome

        data object TimedOut : ChildExecutionOutcome
        data object Interrupted : ChildExecutionOutcome
        data object LaunchFailed : ChildExecutionOutcome
        data class TimeoutCleanupFailed(val child: Process) : ChildExecutionOutcome
        data class InterruptedCleanupFailed(val child: Process) : ChildExecutionOutcome
        data class ProcessCleanupFailed(val child: Process) : ChildExecutionOutcome
    }

    @Synchronized
    fun coreStartReadiness(): CoreValidationWorkerReadiness = when (val state = workerState) {
        ValidationWorkerState.Ready -> CoreValidationWorkerReadiness.Ready
        is ValidationWorkerState.CleanupPending -> when (
            runCatching { state.child.waitFor(0L, TimeUnit.MILLISECONDS) }
                .getOrDefault(false)
        ) {
            true -> {
                workerState = ValidationWorkerState.Ready
                CoreValidationWorkerReadiness.Ready
            }
            false -> CoreValidationWorkerReadiness.CleanupPending
        }
    }

    @Synchronized
    fun validate(
        request: CoreConfigurationValidationRequestWire,
    ): CoreConfigurationValidationOutcomeWire {
        when (coreStartReadiness()) {
            CoreValidationWorkerReadiness.Ready -> Unit
            CoreValidationWorkerReadiness.CleanupPending ->
                return CoreConfigurationValidationOutcomeWire.ValidationCleanupPending
        }
        val initialTime = elapsedRealtimeMillis()
        val initialDeadline = deadlineOutcome(request.timeoutMillis, initialTime)
        if (initialDeadline != null) return initialDeadline
        val deadlineElapsedRealtimeMillis = initialTime + request.timeoutMillis

        val paths = when (val validation = validatePathsAndDigest(request)) {
            PathValidation.BinaryUnavailable ->
                return CoreConfigurationValidationOutcomeWire.BinaryUnavailable
            PathValidation.CandidateMissing ->
                return CoreConfigurationValidationOutcomeWire.CandidateMissing
            PathValidation.PathRejected ->
                return CoreConfigurationValidationOutcomeWire.CandidatePathRejected
            PathValidation.DigestMismatch ->
                return CoreConfigurationValidationOutcomeWire.CandidateDigestMismatch
            PathValidation.IdentityDigestMismatch ->
                return CoreConfigurationValidationOutcomeWire.CandidateIdentityMismatch
            is PathValidation.Accepted -> validation
        }

        val remaining = deadlineElapsedRealtimeMillis - elapsedRealtimeMillis()
        if (remaining <= 0L) return CoreConfigurationValidationOutcomeWire.DeadlineExpired
        if (remaining > MAX_VALIDATION_DEADLINE_MILLIS) {
            return CoreConfigurationValidationOutcomeWire.DeadlineTooFar
        }

        val command = listOf(
            paths.binary.absolutePath,
            "-t",
            "-f",
            paths.candidate.absolutePath,
            "-d",
            paths.workingDirectory.absolutePath,
        )
        val processBuilder = ProcessBuilder(command)
            .directory(paths.workingDirectory)
            .redirectErrorStream(true)
        return when (
            val child = executeChild(processBuilder, request, paths.candidate, remaining)
        ) {
            is ChildExecutionOutcome.Exited -> when {
                !child.digestMatches -> CoreConfigurationValidationOutcomeWire.CandidateDigestMismatch
                child.exitCode == 0 -> CoreConfigurationValidationOutcomeWire.Validated
                else -> CoreConfigurationValidationOutcomeWire.ConfigurationRejected(child.exitCode)
            }
            ChildExecutionOutcome.TimedOut -> CoreConfigurationValidationOutcomeWire.TimedOut
            ChildExecutionOutcome.Interrupted -> CoreConfigurationValidationOutcomeWire.Interrupted
            ChildExecutionOutcome.LaunchFailed ->
                CoreConfigurationValidationOutcomeWire.ProcessLaunchFailed
            is ChildExecutionOutcome.TimeoutCleanupFailed -> {
                workerState = ValidationWorkerState.CleanupPending(child.child)
                CoreConfigurationValidationOutcomeWire.TimeoutCleanupFailed
            }
            is ChildExecutionOutcome.InterruptedCleanupFailed -> {
                workerState = ValidationWorkerState.CleanupPending(child.child)
                CoreConfigurationValidationOutcomeWire.InterruptedCleanupFailed
            }
            is ChildExecutionOutcome.ProcessCleanupFailed -> {
                workerState = ValidationWorkerState.CleanupPending(child.child)
                CoreConfigurationValidationOutcomeWire.ProcessCleanupFailed
            }
        }
    }

    private fun deadlineOutcome(
        timeoutMillis: Long,
        nowMillis: Long,
    ): CoreConfigurationValidationOutcomeWire? {
        return when {
            timeoutMillis <= 0L -> CoreConfigurationValidationOutcomeWire.DeadlineExpired
            timeoutMillis > MAX_VALIDATION_DEADLINE_MILLIS ||
                nowMillis > Long.MAX_VALUE - timeoutMillis ->
                CoreConfigurationValidationOutcomeWire.DeadlineTooFar
            else -> null
        }
    }

    private fun validatePathsAndDigest(
        request: CoreConfigurationValidationRequestWire,
    ): PathValidation {
        val nativeRoot = canonical(nativeLibraryDirectory) ?: return PathValidation.BinaryUnavailable
        val binary = File(nativeRoot, BINARY_NAME)
        val canonicalBinary = canonical(binary) ?: return PathValidation.BinaryUnavailable
        if (
            canonicalBinary != binary.absoluteFile ||
            !canonicalBinary.isFile ||
            !canonicalBinary.canRead() ||
            !canonicalBinary.canExecute()
        ) {
            return PathValidation.BinaryUnavailable
        }

        val filesRoot = canonical(filesDirectory) ?: return PathValidation.PathRejected
        val requestedWorkingDirectory = canonicalAbsolute(request.workingDirectory)
            ?: return PathValidation.PathRejected
        if (
            requestedWorkingDirectory.path != request.workingDirectory ||
            requestedWorkingDirectory != filesRoot ||
            !requestedWorkingDirectory.isDirectory
        ) {
            return PathValidation.PathRejected
        }

        val candidate = canonicalAbsolute(request.candidatePath)
            ?: return PathValidation.PathRejected
        val configDirectory = candidate.parentFile ?: return PathValidation.PathRejected
        val engineDirectory = configDirectory.parentFile
        if (
            candidate.path != request.candidatePath ||
            configDirectory.name != ENGINE_DIRECTORY_NAME ||
            engineDirectory?.name != ENGINES_DIRECTORY_NAME ||
            !isDescendant(configDirectory, filesRoot) ||
            !configDirectory.isDirectory ||
            !candidateMatchesProfile(candidate.name, request.configurationIdentity.profileId)
        ) {
            return PathValidation.PathRejected
        }
        if (!candidate.isFile || !candidate.canRead()) return PathValidation.CandidateMissing
        if (digest(candidate) != request.candidateDigest.encoded()) {
            return PathValidation.DigestMismatch
        }
        if (request.configurationIdentity.documentRevision != request.candidateDigest.sha256Hex) {
            return PathValidation.IdentityDigestMismatch
        }
        return PathValidation.Accepted(canonicalBinary, candidate, requestedWorkingDirectory)
    }

    private fun executeChild(
        builder: ProcessBuilder,
        request: CoreConfigurationValidationRequestWire,
        candidate: File,
        timeoutMillis: Long,
    ): ChildExecutionOutcome {
        val child = try {
            processStarter.start(builder)
        } catch (_: IOException) {
            return ChildExecutionOutcome.LaunchFailed
        } catch (_: SecurityException) {
            return ChildExecutionOutcome.LaunchFailed
        }
        val outputDrain = try {
            startOutputDrain(child)
        } catch (_: SecurityException) {
            return when (terminateAndReap(child)) {
                ProcessCleanupOutcome.Reaped -> ChildExecutionOutcome.LaunchFailed
                ProcessCleanupOutcome.StillRunning -> ChildExecutionOutcome.ProcessCleanupFailed(child)
            }
        }

        val execution = try {
            when (waitForExit(child, timeoutMillis)) {
                ProcessWaitOutcome.Exited -> {
                    val exitCode = try {
                        child.exitValue()
                    } catch (_: IllegalThreadStateException) {
                        return when (terminateAndReap(child)) {
                            ProcessCleanupOutcome.Reaped -> ChildExecutionOutcome.LaunchFailed
                            ProcessCleanupOutcome.StillRunning ->
                                ChildExecutionOutcome.ProcessCleanupFailed(child)
                        }
                    }
                    ChildExecutionOutcome.Exited(
                        exitCode = exitCode,
                        digestMatches = runCatching { digest(candidate) == request.candidateDigest.encoded() }
                            .getOrDefault(false),
                    )
                }
                ProcessWaitOutcome.TimedOut -> when (terminateAndReap(child)) {
                    ProcessCleanupOutcome.Reaped -> ChildExecutionOutcome.TimedOut
                    ProcessCleanupOutcome.StillRunning ->
                        ChildExecutionOutcome.TimeoutCleanupFailed(child)
                }
            }
        } catch (_: InterruptedException) {
            val cleanup = terminateAndReap(child)
            Thread.currentThread().interrupt()
            when (cleanup) {
                ProcessCleanupOutcome.Reaped -> ChildExecutionOutcome.Interrupted
                ProcessCleanupOutcome.StillRunning ->
                    ChildExecutionOutcome.InterruptedCleanupFailed(child)
            }
        } catch (_: SecurityException) {
            when (terminateAndReap(child)) {
                ProcessCleanupOutcome.Reaped -> ChildExecutionOutcome.LaunchFailed
                ProcessCleanupOutcome.StillRunning -> ChildExecutionOutcome.ProcessCleanupFailed(child)
            }
        } finally {
            joinOutputDrain(outputDrain)
        }
        return execution
    }

    private fun startOutputDrain(child: Process): Thread = Thread({
        child.inputStream.use { input ->
            val buffer = ByteArray(DEFAULT_BUFFER_SIZE)
            while (input.read(buffer) >= 0) Unit
        }
    }, "mihomo-config-validation-output").apply {
        isDaemon = true
        start()
    }

    private fun joinOutputDrain(thread: Thread) {
        try {
            thread.join(OUTPUT_DRAIN_JOIN_MILLIS)
        } catch (_: InterruptedException) {
            Thread.currentThread().interrupt()
        }
    }

    private fun waitForExit(child: Process, timeoutMillis: Long): ProcessWaitOutcome =
        when (child.waitFor(timeoutMillis, TimeUnit.MILLISECONDS)) {
            true -> ProcessWaitOutcome.Exited
            false -> ProcessWaitOutcome.TimedOut
        }

    private fun terminateAndReap(child: Process): ProcessCleanupOutcome {
        if (!child.isAlive) return ProcessCleanupOutcome.Reaped
        runCatching { child.destroy() }
        val graceful = runCatching {
            waitForExit(child, GRACEFUL_TERMINATION_MILLIS)
        }.getOrDefault(ProcessWaitOutcome.TimedOut)
        if (graceful == ProcessWaitOutcome.Exited) return ProcessCleanupOutcome.Reaped
        runCatching { child.destroyForcibly() }
        val forced = runCatching {
            waitForExit(child, FORCED_REAP_MILLIS)
        }.getOrDefault(ProcessWaitOutcome.TimedOut)
        return when {
            forced == ProcessWaitOutcome.Exited || !child.isAlive -> ProcessCleanupOutcome.Reaped
            else -> ProcessCleanupOutcome.StillRunning
        }
    }

    private fun digest(file: File): String {
        val sha256 = MessageDigest.getInstance("SHA-256")
        file.inputStream().buffered().use { input ->
            val buffer = ByteArray(DEFAULT_BUFFER_SIZE)
            while (true) {
                val count = input.read(buffer)
                if (count < 0) break
                sha256.update(buffer, 0, count)
            }
        }
        val hex = buildString(64) {
            for (byte in sha256.digest()) {
                val unsigned = byte.toInt() and 0xff
                append(HEX[unsigned ushr 4])
                append(HEX[unsigned and 0x0f])
            }
        }
        return "sha256:$hex"
    }

    private fun canonical(file: File): File? = runCatching { file.canonicalFile }.getOrNull()

    private fun canonicalAbsolute(path: String): File? = runCatching {
        File(path).takeIf(File::isAbsolute)?.canonicalFile
    }.getOrNull()

    private fun isDescendant(path: File, root: File): Boolean =
        path.path.startsWith(root.path + File.separator)

    private fun candidateMatchesProfile(fileName: String, profileId: String): Boolean {
        val targetName = "$profileId.yaml"
        if (fileName == targetName) return true
        val temporaryPrefix = ".$targetName.tmp-"
        val temporarySuffix = fileName.removePrefix(temporaryPrefix)
        return fileName.startsWith(temporaryPrefix) &&
            temporarySuffix.isNotEmpty() &&
            temporarySuffix.all { it.isDigit() || it == '-' }
    }

    private companion object {
        const val BINARY_NAME = "libmihomo.so"
        const val ENGINES_DIRECTORY_NAME = "engines"
        const val ENGINE_DIRECTORY_NAME = "mihomo"
        const val MAX_VALIDATION_DEADLINE_MILLIS = 30_000L
        const val GRACEFUL_TERMINATION_MILLIS = 250L
        const val FORCED_REAP_MILLIS = 750L
        const val OUTPUT_DRAIN_JOIN_MILLIS = 250L
        const val HEX = "0123456789abcdef"
    }
}

internal enum class CoreValidationWorkerReadiness {
    Ready,
    CleanupPending,
}

internal class CoreConfigurationValidationHost(
    private val processRole: () -> AndroidProcessRole,
    private val controller: MihomoProcessController,
) {
    fun validate(requestJson: String): CoreConfigurationValidationReplyWire {
        val request = when (val decoded = CoreConfigurationValidationRequestWire.decode(requestJson)) {
            is CoreConfigurationValidationRequestDecode.Accepted -> decoded.request
            is CoreConfigurationValidationRequestDecode.InvalidRequest -> {
                return CoreConfigurationValidationReplyWire.InvalidRequest(decoded.reason)
            }
        }
        val outcome = when (processRole()) {
            AndroidProcessRole.VpnOwner -> controller.validateConfiguration(request)
            AndroidProcessRole.Ui,
            is AndroidProcessRole.Unsupported ->
                CoreConfigurationValidationOutcomeWire.NotVpnOwner
        }
        return CoreConfigurationValidationReplyWire.Receipt(request, outcome)
    }
}

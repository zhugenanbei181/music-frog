package com.musicfrog.infiltrator

import java.math.BigInteger
import org.json.JSONObject

internal data class AndroidChildProcessIdentityWire(
    val pid: Long,
    val startTimeTicks: BigInteger,
) {
    fun toJson(): JSONObject = JSONObject()
        .put("pid", pid)
        .put("start_time_ticks", startTimeTicks)
}

internal data class AndroidOwnedCoreProcessWire(
    val context: CoreProcessStartContextWire,
    val child: AndroidChildProcessIdentityWire,
) {
    fun toJson(): JSONObject = JSONObject()
        .put("context", context.toManagedJson())
        .put("child", child.toJson())

    companion object {
        fun decode(raw: String): AndroidOwnedCoreProcessDecode = runCatching {
            val json = JSONObject(raw)
            require(json.keys().asSequence().toSet() == setOf("context", "child"))
            val decodedContext = CoreProcessStartContextWire.decode(
                json.getJSONObject("context").toString(),
            )
            val context = when (decodedContext) {
                is CoreProcessStartContextDecode.Accepted -> decodedContext.context
                is CoreProcessStartContextDecode.Rejected -> error(decodedContext.reason)
            }
            val childJson = json.getJSONObject("child")
            require(childJson.keys().asSequence().toSet() == setOf("pid", "start_time_ticks"))
            val pid = childJson.getLong("pid").also { require(it in 1..Int.MAX_VALUE.toLong()) }
            val startTimeTicks = BigInteger(childJson.get("start_time_ticks").toString())
                .also { require(it.signum() > 0) }
            AndroidOwnedCoreProcessDecode.Accepted(
                AndroidOwnedCoreProcessWire(
                    context,
                    AndroidChildProcessIdentityWire(pid, startTimeTicks),
                ),
            )
        }.getOrElse { error ->
            AndroidOwnedCoreProcessDecode.Rejected(error.message ?: "invalid Core child record")
        }
    }
}

internal sealed interface AndroidOwnedCoreProcessDecode {
    data class Accepted(val owned: AndroidOwnedCoreProcessWire) : AndroidOwnedCoreProcessDecode

    data class Rejected(val reason: String) : AndroidOwnedCoreProcessDecode
}

internal enum class AndroidCoreProcessExitEvidenceWire {
    ProcessEntryMissing,
    ProcessStartIdentityChanged,
    ProcessHandleExited,
}

internal enum class AndroidCoreProcessUnavailableReasonWire {
    LaunchReservationIncomplete,
    OwnershipRecordCorrupt,
    ProcessInspectionUnavailable,
    OwnershipRecordUnavailable,
}

internal sealed interface AndroidCoreProcessObservationWire {
    data object NeverStarted : AndroidCoreProcessObservationWire

    data class Running(val owned: AndroidOwnedCoreProcessWire) : AndroidCoreProcessObservationWire

    data class Exited(
        val owned: AndroidOwnedCoreProcessWire,
        val evidence: AndroidCoreProcessExitEvidenceWire,
    ) : AndroidCoreProcessObservationWire

    data class Unavailable(
        val reason: AndroidCoreProcessUnavailableReasonWire,
    ) : AndroidCoreProcessObservationWire

    fun toJson(): String = when (this) {
        NeverStarted -> JSONObject.quote("NeverStarted")
        is Running -> JSONObject().put("Running", JSONObject().put("owned", owned.toJson())).toString()
        is Exited -> JSONObject()
            .put(
                "Exited",
                JSONObject()
                    .put("owned", owned.toJson())
                    .put("evidence", evidence.name),
            )
            .toString()
        is Unavailable -> JSONObject()
            .put("Unavailable", JSONObject().put("reason", reason.name))
            .toString()
    }
}

internal sealed interface StoredAndroidCoreProcessRecord {
    data object Missing : StoredAndroidCoreProcessRecord

    data class LaunchReserved(val context: CoreProcessStartContextWire) : StoredAndroidCoreProcessRecord

    data class Owned(val process: AndroidOwnedCoreProcessWire) : StoredAndroidCoreProcessRecord

    data object Corrupt : StoredAndroidCoreProcessRecord
}

internal sealed interface AndroidChildProcessInspection {
    data object ExactChildRunning : AndroidChildProcessInspection

    data class Exited(val evidence: AndroidCoreProcessExitEvidenceWire) : AndroidChildProcessInspection

    data object Unavailable : AndroidChildProcessInspection
}

internal sealed interface AndroidCurrentThreadChildSnapshot {
    data class Captured(
        val processId: Long,
        val threadId: Long,
        val childIds: Set<Long>,
    ) : AndroidCurrentThreadChildSnapshot

    data object Unavailable : AndroidCurrentThreadChildSnapshot
}

internal fun interface AndroidChildProcessInspector {
    fun inspect(child: AndroidChildProcessIdentityWire): AndroidChildProcessInspection

    fun capture(pid: Long): AndroidChildProcessCapture = AndroidChildProcessCapture.Unavailable

    fun snapshotCurrentThreadChildren(): AndroidCurrentThreadChildSnapshot =
        AndroidCurrentThreadChildSnapshot.Unavailable

    fun captureStartedChild(
        snapshot: AndroidCurrentThreadChildSnapshot,
    ): AndroidChildProcessCapture = AndroidChildProcessCapture.Unavailable
}

internal sealed interface AndroidChildProcessCapture {
    data class Identified(val child: AndroidChildProcessIdentityWire) : AndroidChildProcessCapture

    data object Exited : AndroidChildProcessCapture

    data object Unavailable : AndroidChildProcessCapture
}

internal enum class AndroidCoreProcessRecordMutationOutcome {
    Committed,
    Unavailable,
}

internal enum class AndroidChildProcessSignalOutcome {
    Sent,
    Unavailable,
}

internal enum class AndroidChildProcessSignal {
    Graceful,
    Force,
}

internal fun interface AndroidChildProcessSignalPort {
    fun signal(pid: Int, signal: AndroidChildProcessSignal): AndroidChildProcessSignalOutcome
}

internal class ProcAndroidChildProcessInspector(
    private val procDirectory: java.io.File = java.io.File("/proc"),
    private val currentProcessId: () -> Long = { android.os.Process.myPid().toLong() },
    private val currentThreadId: () -> Long = { android.os.Process.myTid().toLong() },
) : AndroidChildProcessInspector {
    override fun inspect(child: AndroidChildProcessIdentityWire): AndroidChildProcessInspection {
        val stat = readStat(child.pid)
        if (stat === ProcStatRead.Missing) return AndroidChildProcessInspection.Exited(
            AndroidCoreProcessExitEvidenceWire.ProcessEntryMissing,
        )
        if (stat === ProcStatRead.Unavailable) return AndroidChildProcessInspection.Unavailable
        stat as ProcStatRead.Present
        if (stat.state == 'Z' || stat.state == 'X') {
            return AndroidChildProcessInspection.Exited(
                AndroidCoreProcessExitEvidenceWire.ProcessHandleExited,
            )
        }
        return if (stat.startTimeTicks == child.startTimeTicks) {
            AndroidChildProcessInspection.ExactChildRunning
        } else {
            AndroidChildProcessInspection.Exited(
                AndroidCoreProcessExitEvidenceWire.ProcessStartIdentityChanged,
            )
        }
    }

    override fun capture(pid: Long): AndroidChildProcessCapture {
        if (pid !in 1..Int.MAX_VALUE.toLong()) return AndroidChildProcessCapture.Unavailable
        return when (val stat = readStat(pid)) {
            ProcStatRead.Missing -> AndroidChildProcessCapture.Exited
            ProcStatRead.Unavailable -> AndroidChildProcessCapture.Unavailable
            is ProcStatRead.Present -> if (stat.state == 'Z' || stat.state == 'X') {
                AndroidChildProcessCapture.Exited
            } else {
                AndroidChildProcessCapture.Identified(
                    AndroidChildProcessIdentityWire(pid, stat.startTimeTicks),
                )
            }
        }
    }

    override fun snapshotCurrentThreadChildren(): AndroidCurrentThreadChildSnapshot {
        val processId = runCatching { currentProcessId().also { require(it > 0L) } }
            .getOrElse { return AndroidCurrentThreadChildSnapshot.Unavailable }
        val threadId = runCatching { currentThreadId().also { require(it > 0L) } }
            .getOrElse { return AndroidCurrentThreadChildSnapshot.Unavailable }
        return when (val children = readCurrentThreadChildren(processId, threadId)) {
            CurrentThreadChildrenRead.Unavailable -> AndroidCurrentThreadChildSnapshot.Unavailable
            is CurrentThreadChildrenRead.Available -> AndroidCurrentThreadChildSnapshot.Captured(
                processId,
                threadId,
                children.childIds,
            )
        }
    }

    // Process.pid() is newer than this app's minSdk; bind the record to the
    // unique child added by the ProcessBuilder caller thread and recheck it.
    override fun captureStartedChild(
        snapshot: AndroidCurrentThreadChildSnapshot,
    ): AndroidChildProcessCapture = when (snapshot) {
        AndroidCurrentThreadChildSnapshot.Unavailable -> AndroidChildProcessCapture.Unavailable
        is AndroidCurrentThreadChildSnapshot.Captured -> when (
            val currentChildren = readCurrentThreadChildren(snapshot.processId, snapshot.threadId)
        ) {
            CurrentThreadChildrenRead.Unavailable -> AndroidChildProcessCapture.Unavailable
            is CurrentThreadChildrenRead.Available -> {
                val newChildren = currentChildren.childIds - snapshot.childIds
                val candidate = when (newChildren.size) {
                    1 -> capture(newChildren.single())
                    0 -> AndroidChildProcessCapture.Unavailable
                    else -> AndroidChildProcessCapture.Unavailable
                }
                verifyCapturedChild(snapshot, candidate)
            }
        }
    }

    private fun verifyCapturedChild(
        snapshot: AndroidCurrentThreadChildSnapshot.Captured,
        candidate: AndroidChildProcessCapture,
    ): AndroidChildProcessCapture = when (candidate) {
        is AndroidChildProcessCapture.Identified -> when (
            val currentChildren = readCurrentThreadChildren(snapshot.processId, snapshot.threadId)
        ) {
            CurrentThreadChildrenRead.Unavailable -> AndroidChildProcessCapture.Unavailable
            is CurrentThreadChildrenRead.Available -> when (
                candidate.child.pid in currentChildren.childIds
            ) {
                true -> when (inspect(candidate.child)) {
                    AndroidChildProcessInspection.ExactChildRunning -> candidate
                    is AndroidChildProcessInspection.Exited -> AndroidChildProcessCapture.Exited
                    AndroidChildProcessInspection.Unavailable -> AndroidChildProcessCapture.Unavailable
                }
                false -> AndroidChildProcessCapture.Exited
            }
        }
        AndroidChildProcessCapture.Exited -> AndroidChildProcessCapture.Exited
        AndroidChildProcessCapture.Unavailable -> AndroidChildProcessCapture.Unavailable
    }

    private fun readCurrentThreadChildren(
        processId: Long,
        threadId: Long,
    ): CurrentThreadChildrenRead = runCatching {
        val path = java.io.File(procDirectory, "$processId/task/$threadId/children")
        val contents = path.readText().trim()
        val childIds = if (contents.isEmpty()) {
            emptySet()
        } else {
            contents.split(Regex("\\s+")).map { childId ->
                childId.toLong().also { require(it > 0L) }
            }.toSet()
        }
        CurrentThreadChildrenRead.Available(childIds)
    }.getOrElse { CurrentThreadChildrenRead.Unavailable }

    private sealed interface CurrentThreadChildrenRead {
        data class Available(val childIds: Set<Long>) : CurrentThreadChildrenRead

        data object Unavailable : CurrentThreadChildrenRead
    }

    private fun readStat(pid: Long): ProcStatRead {
        val statFile = java.io.File(procDirectory, "$pid/stat")
        val stat = try {
            java.nio.file.Files.newInputStream(statFile.toPath()).use { input ->
                input.readBytes().toString(Charsets.UTF_8)
            }
        } catch (_: java.nio.file.NoSuchFileException) {
            return ProcStatRead.Missing
        } catch (_: java.io.IOException) {
            return ProcStatRead.Unavailable
        } catch (_: SecurityException) {
            return ProcStatRead.Unavailable
        }
        val commandEnd = stat.lastIndexOf(") ")
        if (commandEnd < 0) return ProcStatRead.Unavailable
        val fields = stat.substring(commandEnd + 2).trim().split(Regex("\\s+"))
        val state = fields.getOrNull(0)?.singleOrNull() ?: return ProcStatRead.Unavailable
        val startTimeTicks = fields.getOrNull(PROC_START_TIME_FIELD_INDEX)
            ?.let { runCatching { BigInteger(it) }.getOrNull() }
            ?.takeIf { it.signum() > 0 }
            ?: return ProcStatRead.Unavailable
        return ProcStatRead.Present(state, startTimeTicks)
    }

    private sealed interface ProcStatRead {
        data object Missing : ProcStatRead

        data object Unavailable : ProcStatRead

        data class Present(val state: Char, val startTimeTicks: BigInteger) : ProcStatRead
    }

    private companion object {
        // /proc/<pid>/stat fields start at field 3 after the command name.
        const val PROC_START_TIME_FIELD_INDEX = 19
    }
}

internal class AndroidCoreProcessRecordStore(private val filesDirectory: java.io.File) {
    private val recordFile = java.io.File(filesDirectory, RECORD_FILE_NAME)

    @Synchronized
    fun read(): StoredAndroidCoreProcessRecord {
        if (!recordFile.exists()) return StoredAndroidCoreProcessRecord.Missing
        return runCatching {
            val root = JSONObject(recordFile.readText())
            require(root.keys().asSequence().toSet() == setOf("schema_version", "record"))
            require(root.getInt("schema_version") == RECORD_SCHEMA_VERSION)
            val record = root.getJSONObject("record")
            when (val variant = record.keys().asSequence().singleOrNull()) {
                "LaunchReserved" -> {
                    val decodedContext = CoreProcessStartContextWire.decode(
                        JSONObject().put("Managed", record.getJSONObject(variant)).toString(),
                    )
                    val context = when (decodedContext) {
                        is CoreProcessStartContextDecode.Accepted -> decodedContext.context
                        is CoreProcessStartContextDecode.Rejected -> error(decodedContext.reason)
                    }
                    StoredAndroidCoreProcessRecord.LaunchReserved(context)
                }
                "Owned" -> {
                    val decodedOwned = AndroidOwnedCoreProcessWire.decode(
                        record.getJSONObject(variant).toString(),
                    )
                    when (decodedOwned) {
                        is AndroidOwnedCoreProcessDecode.Accepted ->
                            StoredAndroidCoreProcessRecord.Owned(decodedOwned.owned)
                        is AndroidOwnedCoreProcessDecode.Rejected -> error(decodedOwned.reason)
                    }
                }
                else -> error("unknown Android Core ownership record variant")
            }
        }.getOrElse { StoredAndroidCoreProcessRecord.Corrupt }
    }

    @Synchronized
    fun writeReservation(
        context: CoreProcessStartContextWire,
    ): AndroidCoreProcessRecordMutationOutcome = write(
        JSONObject().put("LaunchReserved", context.toManagedJson().getJSONObject("Managed")),
    )

    @Synchronized
    fun writeOwned(
        process: AndroidOwnedCoreProcessWire,
    ): AndroidCoreProcessRecordMutationOutcome = write(
        JSONObject().put("Owned", process.toJson()),
    )

    @Synchronized
    fun clear(): AndroidCoreProcessRecordMutationOutcome = runCatching {
        if (recordFile.exists()) require(recordFile.delete())
        AndroidCoreProcessRecordMutationOutcome.Committed
    }.getOrDefault(AndroidCoreProcessRecordMutationOutcome.Unavailable)

    private fun write(record: JSONObject): AndroidCoreProcessRecordMutationOutcome = runCatching {
        require(filesDirectory.isDirectory || filesDirectory.mkdirs())
        val root = JSONObject()
            .put("schema_version", RECORD_SCHEMA_VERSION)
            .put("record", record)
        val temporary = java.io.File(filesDirectory, "$RECORD_FILE_NAME.tmp")
        java.io.FileOutputStream(temporary).use { output ->
            output.write(root.toString().toByteArray(Charsets.UTF_8))
            output.fd.sync()
        }
        java.nio.file.Files.move(
            temporary.toPath(),
            recordFile.toPath(),
            java.nio.file.StandardCopyOption.ATOMIC_MOVE,
            java.nio.file.StandardCopyOption.REPLACE_EXISTING,
        )
        AndroidCoreProcessRecordMutationOutcome.Committed
    }.getOrDefault(AndroidCoreProcessRecordMutationOutcome.Unavailable)

    private companion object {
        const val RECORD_FILE_NAME = "mihomo-core-owner.json"
        const val RECORD_SCHEMA_VERSION = 1
    }
}

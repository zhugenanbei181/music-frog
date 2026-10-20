package com.musicfrog.infiltrator

import org.junit.Assert.assertEquals
import org.junit.Test
import java.io.ByteArrayInputStream
import java.io.ByteArrayOutputStream
import java.io.File
import java.io.InputStream
import java.io.OutputStream
import java.math.BigInteger
import java.nio.file.Files
import java.security.MessageDigest

class AndroidCoreProcessRecordStoreTest {
    @Test
    fun recoveryReceiptWireCodesAreExhaustiveAndStable() {
        assertEquals(
            listOf(0, 1, 2, 3, 4, 5),
            AndroidCoreProcessRecoveryReceiptWire.entries.map { it.wireCode },
        )
    }

    @Test
    fun reservationAndOwnedChildRecordRoundTripWithFullOwnerFence() = withDirectory { directory ->
        val store = AndroidCoreProcessRecordStore(directory)
        val context = managedContext()

        assertEquals(AndroidCoreProcessRecordMutationOutcome.Committed, store.writeReservation(context))
        assertEquals(
            StoredAndroidCoreProcessRecord.LaunchReserved(context),
            store.read(),
        )

        val owned = AndroidOwnedCoreProcessWire(
            context,
            AndroidChildProcessIdentityWire(321L, BigInteger("987654")),
        )
        assertEquals(AndroidCoreProcessRecordMutationOutcome.Committed, store.writeOwned(owned))
        assertEquals(StoredAndroidCoreProcessRecord.Owned(owned), store.read())
        assertEquals(AndroidCoreProcessRecordMutationOutcome.Committed, store.clear())
        assertEquals(StoredAndroidCoreProcessRecord.Missing, store.read())
    }

    @Test
    fun malformedOrUnknownRecordFailsClosed() = withDirectory { directory ->
        val store = AndroidCoreProcessRecordStore(directory)
        File(directory, "mihomo-core-owner.json").writeText("{invalid")

        assertEquals(StoredAndroidCoreProcessRecord.Corrupt, store.read())
        var spawns = 0
        val controller = CoreProcessController(
            nativeLibraryDirectory = File(directory, "native"),
            filesDirectory = directory,
            processStarter = CoreProcessStarter {
                spawns += 1
                FakeProcess()
            },
            recordStore = store,
            childInspector = ExactChildInspector,
        )
        assertEquals(
            CoreProcessStartReceipt.OwnershipRecordUnavailable,
            controller.start(launchSpec(directory), managedContext()),
        )
        assertEquals(0, spawns)
    }

    @Test
    fun procIdentityDistinguishesExactChildFromMissingAndReusedPid() = withDirectory { proc ->
        val inspector = ProcAndroidChildProcessInspector(proc)
        val child = AndroidChildProcessIdentityWire(321L, BigInteger("987654"))
        val stat = File(proc, "321/stat")
        stat.parentFile?.mkdirs()

        stat.writeText(procStat("S", "987654"))
        assertEquals(AndroidChildProcessInspection.ExactChildRunning, inspector.inspect(child))

        stat.writeText(procStat("S", "987655"))
        assertEquals(
            AndroidChildProcessInspection.Exited(
                AndroidCoreProcessExitEvidenceWire.ProcessStartIdentityChanged,
            ),
            inspector.inspect(child),
        )

        stat.delete()
        assertEquals(
            AndroidChildProcessInspection.Exited(AndroidCoreProcessExitEvidenceWire.ProcessEntryMissing),
            inspector.inspect(child),
        )
    }

    @Test
    fun procSnapshotIdentifiesOnlyTheNewChildOfTheLaunchingThread() = withDirectory { proc ->
        val children = File(proc, "101/task/202/children").apply {
            parentFile?.mkdirs()
            writeText("111")
        }
        File(proc, "444/stat").apply {
            parentFile?.mkdirs()
            writeText(procStat("S", "987654"))
        }
        val inspector = ProcAndroidChildProcessInspector(
            procDirectory = proc,
            currentProcessId = { 101L },
            currentThreadId = { 202L },
        )
        val snapshot = inspector.snapshotCurrentThreadChildren()
        assertEquals(
            AndroidCurrentThreadChildSnapshot.Captured(101L, 202L, setOf(111L)),
            snapshot,
        )

        children.writeText("111 444")
        assertEquals(
            AndroidChildProcessCapture.Identified(
                AndroidChildProcessIdentityWire(444L, BigInteger("987654")),
            ),
            inspector.captureStartedChild(snapshot),
        )

        val nextSnapshot = inspector.snapshotCurrentThreadChildren()
        children.writeText("111 444 555 666")
        assertEquals(
            AndroidChildProcessCapture.Unavailable,
            inspector.captureStartedChild(nextSnapshot),
        )
    }

    @Test
    fun unavailableLaunchThreadSnapshotRejectsBeforeProcessSpawn() = withDirectory { root ->
        val native = File(root, "native").apply { mkdirs() }
        File(native, "libmihomo.so").apply {
            writeBytes(byteArrayOf(0x7f, 0x45, 0x4c, 0x46))
            setExecutable(true)
        }
        val files = File(root, "files").apply { mkdirs() }
        var spawns = 0
        val controller = CoreProcessController(
            nativeLibraryDirectory = native,
            filesDirectory = files,
            processStarter = CoreProcessStarter {
                spawns += 1
                FakeProcess()
            },
            childInspector = object : AndroidChildProcessInspector {
                override fun inspect(child: AndroidChildProcessIdentityWire): AndroidChildProcessInspection =
                    AndroidChildProcessInspection.Unavailable

                override fun snapshotCurrentThreadChildren(): AndroidCurrentThreadChildSnapshot =
                    AndroidCurrentThreadChildSnapshot.Unavailable
            },
        )

        assertEquals(
            CoreProcessStartReceipt.OwnershipRecordUnavailable,
            controller.start(launchSpec(files), managedContext()),
        )
        assertEquals(0, spawns)
    }

    @Test
    fun controllerPersistsReservationBeforeSpawnAndThenExactChildRecord() = withDirectory { root ->
        val native = File(root, "native").apply { mkdirs() }
        File(native, "libmihomo.so").apply {
            writeBytes(byteArrayOf(0x7f, 0x45, 0x4c, 0x46))
            setExecutable(true)
        }
        val files = File(root, "files").apply { mkdirs() }
        val store = AndroidCoreProcessRecordStore(files)
        val context = managedContext()
        val child = FakeProcess()
        val controller = CoreProcessController(
            nativeLibraryDirectory = native,
            filesDirectory = files,
            processStarter = CoreProcessStarter {
                assertEquals(
                    StoredAndroidCoreProcessRecord.LaunchReserved(context),
                    store.read(),
                )
                child
            },
            recordStore = store,
            childInspector = ExactChildInspector,
        )

        val receipt = controller.start(launchSpec(files), context)

        assertEquals(CoreProcessStartReceipt.Started, receipt)
        val owned = AndroidOwnedCoreProcessWire(
            context,
            AndroidChildProcessIdentityWire(321L, BigInteger("987654")),
        )
        assertEquals(StoredAndroidCoreProcessRecord.Owned(owned), store.read())
        assertEquals(AndroidCoreProcessObservationWire.Running(owned), controller.observation())
    }

    @Test
    fun restartedControllerReportsPreviousExitInsteadOfCallingItNeverStarted() = withDirectory { root ->
        val files = File(root, "files").apply { mkdirs() }
        val store = AndroidCoreProcessRecordStore(files)
        val owned = AndroidOwnedCoreProcessWire(
            managedContext(),
            AndroidChildProcessIdentityWire(321L, BigInteger("987654")),
        )
        assertEquals(AndroidCoreProcessRecordMutationOutcome.Committed, store.writeOwned(owned))

        val controller = CoreProcessController(
            nativeLibraryDirectory = File(root, "native"),
            filesDirectory = files,
            recordStore = store,
            childInspector = FixedChildInspector(
                AndroidChildProcessInspection.Exited(
                    AndroidCoreProcessExitEvidenceWire.ProcessStartIdentityChanged,
                ),
            ),
        )

        assertEquals(
            AndroidCoreProcessObservationWire.Exited(
                owned,
                AndroidCoreProcessExitEvidenceWire.ProcessStartIdentityChanged,
            ),
            controller.observation(),
        )
        assertEquals(
            CoreProcessStartReceipt.PreviousChildExitPending,
            controller.start(launchSpec(files), owned.context),
        )
        assertEquals(CoreProcessStopReceipt.AlreadyStopped, controller.stop())
        assertEquals(StoredAndroidCoreProcessRecord.Owned(owned), store.read())
    }

    @Test
    fun recoveredExitRecordIsConsumedOnlyForTheExactExitedManagedContext() = withDirectory { root ->
        val files = File(root, "files").apply { mkdirs() }
        val store = AndroidCoreProcessRecordStore(files)
        val owned = AndroidOwnedCoreProcessWire(
            managedContext(),
            AndroidChildProcessIdentityWire(321L, BigInteger("987654")),
        )
        assertEquals(AndroidCoreProcessRecordMutationOutcome.Committed, store.writeOwned(owned))
        val controller = CoreProcessController(
            nativeLibraryDirectory = File(root, "native"),
            filesDirectory = files,
            recordStore = store,
            childInspector = FixedChildInspector(
                AndroidChildProcessInspection.Exited(
                    AndroidCoreProcessExitEvidenceWire.ProcessStartIdentityChanged,
                ),
            ),
        )

        assertEquals(
            AndroidCoreProcessRecoveryReceiptWire.ContextMismatch,
            controller.consumeRecoveredExit(
                owned.context.copy(sessionToken = BigInteger("43")),
            ),
        )
        assertEquals(StoredAndroidCoreProcessRecord.Owned(owned), store.read())
        assertEquals(
            AndroidCoreProcessRecoveryReceiptWire.Consumed,
            controller.consumeRecoveredExit(owned.context),
        )
        assertEquals(StoredAndroidCoreProcessRecord.Missing, store.read())
        assertEquals(AndroidCoreProcessObservationWire.NeverStarted, controller.observation())
    }

    @Test
    fun recoveredExitConsumptionRefusesALiveExactChild() = withDirectory { root ->
        val files = File(root, "files").apply { mkdirs() }
        val store = AndroidCoreProcessRecordStore(files)
        val owned = AndroidOwnedCoreProcessWire(
            managedContext(),
            AndroidChildProcessIdentityWire(321L, BigInteger("987654")),
        )
        assertEquals(AndroidCoreProcessRecordMutationOutcome.Committed, store.writeOwned(owned))
        val controller = CoreProcessController(
            nativeLibraryDirectory = File(root, "native"),
            filesDirectory = files,
            recordStore = store,
            childInspector = ExactChildInspector,
        )

        assertEquals(
            AndroidCoreProcessRecoveryReceiptWire.ChildStillRunning,
            controller.consumeRecoveredExit(owned.context),
        )
        assertEquals(StoredAndroidCoreProcessRecord.Owned(owned), store.read())
    }

    @Test
    fun restartedControllerAdoptsOnlyTheExactLiveChildOwnerContext() = withDirectory { root ->
        val files = File(root, "files").apply { mkdirs() }
        val store = AndroidCoreProcessRecordStore(files)
        val owned = AndroidOwnedCoreProcessWire(
            managedContext(),
            AndroidChildProcessIdentityWire(321L, BigInteger("987654")),
        )
        assertEquals(AndroidCoreProcessRecordMutationOutcome.Committed, store.writeOwned(owned))
        var spawns = 0
        val controller = CoreProcessController(
            nativeLibraryDirectory = File(root, "native"),
            filesDirectory = files,
            processStarter = CoreProcessStarter {
                spawns += 1
                FakeProcess()
            },
            recordStore = store,
            childInspector = ExactChildInspector,
        )

        assertEquals(AndroidCoreProcessObservationWire.Running(owned), controller.observation())
        assertEquals(
            CoreProcessStartReceipt.AlreadyRunning,
            controller.start(launchSpec(files), owned.context),
        )
        assertEquals(
            CoreProcessStartReceipt.OwnerContextMismatch,
            controller.start(
                launchSpec(files),
                owned.context.copy(generation = BigInteger("8")),
            ),
        )
        assertEquals(0, spawns)
    }

    @Test
    fun restoredChildStopUsesTypedSignalPortAndRetainsTerminalEvidence() = withDirectory { root ->
        val files = File(root, "files").apply { mkdirs() }
        val store = AndroidCoreProcessRecordStore(files)
        val owned = AndroidOwnedCoreProcessWire(
            managedContext(),
            AndroidChildProcessIdentityWire(321L, BigInteger("987654")),
        )
        assertEquals(AndroidCoreProcessRecordMutationOutcome.Committed, store.writeOwned(owned))
        val observations = listOf(
            AndroidChildProcessInspection.ExactChildRunning,
            AndroidChildProcessInspection.ExactChildRunning,
            AndroidChildProcessInspection.ExactChildRunning,
            AndroidChildProcessInspection.ExactChildRunning,
            AndroidChildProcessInspection.Exited(
                AndroidCoreProcessExitEvidenceWire.ProcessHandleExited,
            ),
        )
        var observationIndex = 0
        val inspector = object : AndroidChildProcessInspector {
            override fun inspect(child: AndroidChildProcessIdentityWire): AndroidChildProcessInspection = observations[observationIndex++]
        }
        var signalCalls = 0
        val controller = CoreProcessController(
            nativeLibraryDirectory = File(root, "native"),
            filesDirectory = files,
            elapsedRealtimeMillis = { 0L },
            recordStore = store,
            childInspector = inspector,
            childSignalPort = AndroidChildProcessSignalPort { pid, signal ->
                assertEquals(321, pid)
                assertEquals(AndroidChildProcessSignal.Graceful, signal)
                signalCalls += 1
                AndroidChildProcessSignalOutcome.Sent
            },
        )

        assertEquals(CoreProcessStopReceipt.StoppedGracefully, controller.stop())
        assertEquals(1, signalCalls)
        assertEquals(
            AndroidCoreProcessObservationWire.Exited(
                owned,
                AndroidCoreProcessExitEvidenceWire.ProcessHandleExited,
            ),
            controller.observation(),
        )
        assertEquals(StoredAndroidCoreProcessRecord.Owned(owned), store.read())
    }

    private fun withDirectory(block: (File) -> Unit) {
        val directory = Files.createTempDirectory("android-core-owner-record-").toFile()
        try {
            block(directory)
        } finally {
            directory.deleteRecursively()
        }
    }

    private fun managedContext() = CoreProcessStartContextWire(
        CoreIdentityWire("mihomo", "android-active", null),
        BigInteger("7"),
        BigInteger("42"),
    )

    private fun launchSpec(files: File): CoreLaunchSpecWire {
        val configuration = File(files, "config.yaml").apply {
            writeText("mixed-port: 7890\nmode: rule\n")
        }
        val revision = MessageDigest.getInstance("SHA-256")
            .digest(configuration.readBytes())
            .joinToString("") { byte -> "%02x".format(byte.toInt() and 0xff) }
        return CoreLaunchSpecWire(
            CoreLaunchConfigurationIdentityWire("mihomo", "primary", revision),
            CoreLaunchBinaryWire.MihomoBundled,
            files.absolutePath,
            CoreLaunchArgumentsWire.Mihomo(configuration.absolutePath),
        )
    }

    private fun procStat(state: String, startTimeTicks: String): String {
        val fields = listOf(state) + (4..21).map { "0" } + startTimeTicks
        return "321 (mihomo test)) ${fields.joinToString(" ")}"
    }

    private object ExactChildInspector : AndroidChildProcessInspector {
        override fun inspect(child: AndroidChildProcessIdentityWire): AndroidChildProcessInspection =
            AndroidChildProcessInspection.ExactChildRunning

        override fun snapshotCurrentThreadChildren(): AndroidCurrentThreadChildSnapshot =
            AndroidCurrentThreadChildSnapshot.Captured(101L, 202L, emptySet())

        override fun captureStartedChild(snapshot: AndroidCurrentThreadChildSnapshot): AndroidChildProcessCapture =
            AndroidChildProcessCapture.Identified(
                AndroidChildProcessIdentityWire(321L, BigInteger("987654")),
            )
    }

    private class FixedChildInspector(private val result: AndroidChildProcessInspection) : AndroidChildProcessInspector {
        override fun inspect(child: AndroidChildProcessIdentityWire): AndroidChildProcessInspection = result
    }

    private class FakeProcess : Process() {
        private enum class Lifecycle {
            Running,
            Exited,
        }

        private var lifecycle = Lifecycle.Running

        override fun getOutputStream(): OutputStream = ByteArrayOutputStream()
        override fun getInputStream(): InputStream = ByteArrayInputStream(ByteArray(0))
        override fun getErrorStream(): InputStream = ByteArrayInputStream(ByteArray(0))
        override fun waitFor(): Int {
            lifecycle = Lifecycle.Exited
            return 0
        }
        override fun waitFor(timeout: Long, unit: java.util.concurrent.TimeUnit): Boolean = lifecycle == Lifecycle.Exited
        override fun exitValue(): Int = when (lifecycle) {
            Lifecycle.Running -> throw IllegalThreadStateException("fake process is running")
            Lifecycle.Exited -> 0
        }
        override fun destroy() {
            lifecycle = Lifecycle.Exited
        }
        override fun destroyForcibly(): Process {
            lifecycle = Lifecycle.Exited
            return this
        }
        override fun isAlive(): Boolean = lifecycle == Lifecycle.Running
    }
}

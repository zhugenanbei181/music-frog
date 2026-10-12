package com.musicfrog.infiltrator

import java.io.ByteArrayInputStream
import java.io.ByteArrayOutputStream
import java.io.File
import java.io.InputStream
import java.io.OutputStream
import java.math.BigInteger
import java.nio.file.Files
import java.security.MessageDigest
import java.util.concurrent.TimeUnit
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class MihomoCoreConfigurationValidatorTest {
    private enum class BinaryFixture {
        Present,
        Missing,
    }

    @Test
    fun runningChildIsAdoptedOnlyByTheSameTypedOwnerContext() = withFiles { files ->
        val child = FakeProcess(FakeProcessBehavior.Running)
        var starts = 0
        val controller = files.controller {
            starts += 1
            child
        }
        val ownerContext = startContext(generation = 3L, token = BigInteger("88"))

        assertEquals(
            CoreProcessStartReceipt.Started,
            controller.start(files.launchSpec(), ownerContext),
        )
        assertEquals(
            CoreProcessStartReceipt.AlreadyRunning,
            controller.start(files.launchSpec(), ownerContext),
        )
        assertEquals(
            CoreProcessStartReceipt.OwnerContextMismatch,
            controller.start(files.launchSpec(), startContext(generation = 4L, token = BigInteger("89"))),
        )
        assertEquals(1, starts)
    }

    @Test
    fun validationRunsSameAbiBinaryWithOnlyCheckArgumentsAndDoesNotWriteCandidate() = withFiles { files ->
        val contentBefore = files.candidate.readBytes()
        val child = FakeProcess(FakeProcessBehavior.Exit(0))
        val builders = mutableListOf<ProcessBuilder>()
        val validator = files.validator { builder ->
            builders += builder
            child
        }

        assertEquals(
            CoreConfigurationValidationOutcomeWire.Validated,
            validator.validate(files.request()),
        )
        assertEquals(1, builders.size)
        assertEquals(
            listOf(
                files.binary.absolutePath,
                "-t",
                "-f",
                files.candidate.absolutePath,
                "-d",
                files.filesDirectory.absolutePath,
            ),
            builders.single().command(),
        )
        assertEquals(files.filesDirectory, builders.single().directory())
        assertTrue(builders.single().redirectErrorStream())
        assertEquals(contentBefore.toList(), files.candidate.readBytes().toList())
    }

    @Test
    fun missingSameAbiBinaryReturnsTypedUnavailableWithoutLaunchingAChild() = withFiles(
        binaryFixture = BinaryFixture.Missing,
    ) { files ->
        var starts = 0
        val outcome = files.validator {
            starts += 1
            FakeProcess(FakeProcessBehavior.Exit(0))
        }.validate(files.request())

        assertEquals(CoreConfigurationValidationOutcomeWire.BinaryUnavailable, outcome)
        assertEquals(0, starts)
    }

    @Test
    fun activeCoreRejectsValidationUnderTheSharedControllerLock() = withFiles { files ->
        val activeCore = FakeProcess(FakeProcessBehavior.Running)
        var starts = 0
        val controller = files.controller { _ ->
            starts += 1
            if (starts == 1) activeCore else FakeProcess(FakeProcessBehavior.Exit(0))
        }
        assertEquals(
            CoreProcessStartReceipt.Started,
            controller.start(files.launchSpec(), startContext()),
        )
        assertEquals(
            CoreConfigurationValidationOutcomeWire.CoreActive,
            controller.validateConfiguration(files.request()),
        )
        assertEquals(1, starts)
    }

    @Test
    fun deadlineTimeoutDestroysAndReapsTheValidationChild() = withFiles { files ->
        val child = FakeProcess(FakeProcessBehavior.TimeoutThenForceReap)
        val outcome = files.validator { child }.validate(files.request(timeoutMillis = 500L))

        assertEquals(CoreConfigurationValidationOutcomeWire.TimedOut, outcome)
        assertEquals(1, child.destroyCalls)
        assertEquals(1, child.forceDestroyCalls)
        assertFalse(child.isAlive)
        assertEquals(3, child.timedWaitCalls)
    }

    @Test
    fun unreapedValidationChildKeepsNewValidationAndCoreStartFenced() = withFiles { files ->
        val unreapedChild = FakeProcess(FakeProcessBehavior.NeverReaps)
        var starts = 0
        val controller = files.controller {
            starts += 1
            if (starts == 1) unreapedChild else FakeProcess(FakeProcessBehavior.Running)
        }

        assertEquals(
            CoreConfigurationValidationOutcomeWire.TimeoutCleanupFailed,
            controller.validateConfiguration(files.request(timeoutMillis = 500L)),
        )
        assertEquals(
            CoreConfigurationValidationOutcomeWire.ValidationCleanupPending,
            controller.validateConfiguration(files.request()),
        )
        assertEquals(CoreProcessStartReceipt.SpawnFailed, controller.start(files.launchSpec(), startContext()))
        assertEquals(1, starts)

        unreapedChild.reapExternally()
        assertEquals(CoreProcessStartReceipt.Started, controller.start(files.launchSpec(), startContext()))
        assertEquals(2, starts)
    }

    @Test
    fun receiptEchoesRequestIdentityAndCandidateDigest() = withFiles { files ->
        val request = files.request()
        val host = CoreConfigurationValidationHost(
            processRole = { AndroidProcessRole.VpnOwner },
            controller = files.controller { FakeProcess(FakeProcessBehavior.Exit(0)) },
        )

        val response = JSONObject(host.validate(request.toJson().toString()).toJson())
        val receipt = response.getJSONObject("Receipt")
        val echoedRequest = receipt.getJSONObject("request")
        val identity = echoedRequest.getJSONObject("configuration_identity")
        assertEquals(request.requestId, echoedRequest.getLong("request_id"))
        assertEquals(request.timeoutMillis, echoedRequest.getLong("timeout_millis"))
        assertEquals("mihomo", identity.getString("engine_id"))
        assertEquals("primary", identity.getString("profile_id"))
        assertEquals(
            request.candidateDigest.sha256Hex,
            identity.getString("document_revision"),
        )
        assertEquals(files.candidate.absolutePath, echoedRequest.getString("candidate_path"))
        assertEquals(request.candidateDigest.encoded(), echoedRequest.getString("candidate_digest"))
        assertEquals(files.filesDirectory.absolutePath, echoedRequest.getString("working_directory"))
        assertEquals("Validated", receipt.getString("outcome"))
    }

    @Test
    fun nonOwnerRoleRejectsWithoutLaunchingAChild() = withFiles { files ->
        listOf(
            AndroidProcessRole.Ui,
            AndroidProcessRole.Unsupported("com.musicfrog.infiltrator:worker"),
        ).forEach { role ->
            var starts = 0
            val host = CoreConfigurationValidationHost(
                processRole = { role },
                controller = files.controller {
                    starts += 1
                    FakeProcess(FakeProcessBehavior.Exit(0))
                },
            )

            val request = files.request()
            val reply = host.validate(request.toJson().toString())
            assertEquals(
                CoreConfigurationValidationReplyWire.Receipt(
                    request,
                    CoreConfigurationValidationOutcomeWire.NotVpnOwner,
                ),
                reply,
            )
            assertEquals(0, starts)
        }
    }

    @Test
    fun candidateMustMatchTheProfileIdentityAndOwnedEngineDirectory() = withFiles { files ->
        val wrongProfile = files.request(candidateName = "other.yaml")
        val outsideOwnedDirectory = files.request(
            candidate = File(files.filesDirectory, "primary.yaml"),
        )
        var starts = 0
        val validator = files.validator {
            starts += 1
            FakeProcess(FakeProcessBehavior.Exit(0))
        }

        assertEquals(
            CoreConfigurationValidationOutcomeWire.CandidatePathRejected,
            validator.validate(wrongProfile),
        )
        assertEquals(
            CoreConfigurationValidationOutcomeWire.CandidatePathRejected,
            validator.validate(outsideOwnedDirectory),
        )
        assertEquals(0, starts)
    }

    @Test
    fun candidateSupportsAnAppPrivateRedirectedConfigRoot() = withFiles { files ->
        val redirectedDirectory = File(
            files.filesDirectory,
            "cloud/profiles/engines/mihomo",
        ).apply { mkdirs() }
        val redirectedCandidate = File(redirectedDirectory, "primary.yaml").apply {
            writeBytes(files.candidate.readBytes())
        }

        val outcome = files.validator { FakeProcess(FakeProcessBehavior.Exit(0)) }
            .validate(files.request(candidate = redirectedCandidate))

        assertEquals(CoreConfigurationValidationOutcomeWire.Validated, outcome)
    }

    @Test
    fun digestMismatchRejectsBeforeLaunchingAChild() = withFiles { files ->
        val request = files.request()
        files.candidate.appendText("# changed after Rust hashed it\n")
        var starts = 0

        val outcome = files.validator {
            starts += 1
            FakeProcess(FakeProcessBehavior.Exit(0))
        }.validate(request)

        assertEquals(CoreConfigurationValidationOutcomeWire.CandidateDigestMismatch, outcome)
        assertEquals(0, starts)
    }

    @Test
    fun candidateDigestMustMatchTheFrozenProfileRevision() = withFiles { files ->
        val request = files.request().copy(
            configurationIdentity = files.identity(documentRevision = "stale-revision"),
        )
        var starts = 0

        val outcome = files.validator {
            starts += 1
            FakeProcess(FakeProcessBehavior.Exit(0))
        }.validate(request)

        assertEquals(CoreConfigurationValidationOutcomeWire.CandidateIdentityMismatch, outcome)
        assertEquals(0, starts)
    }

    private inline fun withFiles(
        binaryFixture: BinaryFixture = BinaryFixture.Present,
        block: (ValidationFiles) -> Unit,
    ) {
        val root = Files.createTempDirectory("mihomo-validation-").toFile()
        val files = ValidationFiles(root, binaryFixture)
        try {
            block(files)
        } finally {
            root.deleteRecursively()
        }
    }

    private inner class ValidationFiles(root: File, binaryFixture: BinaryFixture) {
        val filesDirectory = File(root, "files").apply { mkdirs() }
        private val configDirectory = File(filesDirectory, "configs/engines/mihomo")
            .apply { mkdirs() }
        val candidate = File(configDirectory, "primary.yaml").apply {
            writeText("mixed-port: 7890\nmode: rule\n")
        }
        private val nativeDirectory = File(root, "native").apply { mkdirs() }
        val binary = File(nativeDirectory, "libmihomo.so").apply {
            when (binaryFixture) {
                BinaryFixture.Present -> {
                    writeBytes(byteArrayOf(0x7f, 0x45, 0x4c, 0x46))
                    setExecutable(true)
                }
                BinaryFixture.Missing -> Unit
            }
        }

        fun request(
            timeoutMillis: Long = 5_000L,
            candidateName: String = this.candidate.name,
            candidate: File = File(this.candidate.parentFile, candidateName),
        ): CoreConfigurationValidationRequestWire = CoreConfigurationValidationRequestWire(
            requestId = 41L,
            timeoutMillis = timeoutMillis,
            configurationIdentity = identity(),
            candidatePath = candidate.absolutePath,
            candidateDigest = when (
                val digest = CoreConfigurationDigestWire.decode(digest(this.candidate))
            ) {
                is CoreConfigurationDigestWireDecode.Accepted -> digest.digest
                CoreConfigurationDigestWireDecode.Rejected -> error("test digest is valid")
            },
            workingDirectory = filesDirectory.absolutePath,
        )

        fun validator(starter: MihomoProcessStarter): MihomoCoreConfigurationValidator =
            MihomoCoreConfigurationValidator(
                nativeLibraryDirectory = nativeDirectory,
                filesDirectory = filesDirectory,
                processStarter = starter,
                elapsedRealtimeMillis = { 100L },
            )

        fun controller(starter: MihomoProcessStarter): MihomoProcessController =
            MihomoProcessController(
                nativeLibraryDirectory = nativeDirectory,
                filesDirectory = filesDirectory,
                processStarter = starter,
                elapsedRealtimeMillis = { 100L },
                recordStore = AndroidCoreProcessRecordStore(filesDirectory),
                childInspector = FixedChildInspector,
            )

        fun launchSpec(): CoreLaunchSpecWire = CoreLaunchSpecWire(
            configurationIdentity = identity(),
            workingDirectory = filesDirectory.absolutePath,
            launchArguments = listOf(
                "-d",
                filesDirectory.absolutePath,
                "-f",
                candidate.absolutePath,
            ),
        )

        fun identity(documentRevision: String = digest(candidate).removePrefix("sha256:")) =
            CoreLaunchConfigurationIdentityWire(
                engineId = "mihomo",
                profileId = "primary",
                documentRevision = documentRevision,
            )
    }

    private fun digest(file: File): String {
        val hex = MessageDigest.getInstance("SHA-256").digest(file.readBytes()).joinToString("") {
            "%02x".format(it.toInt() and 0xff)
        }
        return "sha256:$hex"
    }

    private sealed interface FakeProcessBehavior {
        data class Exit(val code: Int) : FakeProcessBehavior
        data object Running : FakeProcessBehavior
        data object TimeoutThenForceReap : FakeProcessBehavior
        data object NeverReaps : FakeProcessBehavior
    }

    private class FakeProcess(
        private val behavior: FakeProcessBehavior,
    ) : Process() {
        private enum class Lifecycle {
            Running,
            Exited,
        }

        private var lifecycle = when (behavior) {
            is FakeProcessBehavior.Exit -> Lifecycle.Exited
            FakeProcessBehavior.Running,
            FakeProcessBehavior.TimeoutThenForceReap,
            FakeProcessBehavior.NeverReaps -> Lifecycle.Running
        }
        private var exitCode = (behavior as? FakeProcessBehavior.Exit)?.code ?: 0
        var destroyCalls: Int = 0
            private set
        var forceDestroyCalls: Int = 0
            private set
        var timedWaitCalls: Int = 0
            private set

        override fun getOutputStream(): OutputStream = ByteArrayOutputStream()
        override fun getInputStream(): InputStream = ByteArrayInputStream(ByteArray(0))
        override fun getErrorStream(): InputStream = ByteArrayInputStream(ByteArray(0))

        override fun waitFor(): Int {
            lifecycle = Lifecycle.Exited
            return exitCode
        }

        override fun waitFor(timeout: Long, unit: TimeUnit): Boolean {
            timedWaitCalls += 1
            return when (behavior) {
                is FakeProcessBehavior.Exit -> {
                    exitCode = behavior.code
                    lifecycle = Lifecycle.Exited
                    true
                }
                FakeProcessBehavior.Running -> false
                FakeProcessBehavior.TimeoutThenForceReap -> {
                    if (forceDestroyCalls > 0) {
                        lifecycle = Lifecycle.Exited
                        true
                    } else {
                        false
                    }
                }
                FakeProcessBehavior.NeverReaps -> lifecycle == Lifecycle.Exited
            }
        }

        override fun exitValue(): Int = when (lifecycle) {
            Lifecycle.Running -> throw IllegalThreadStateException("fake child is still running")
            Lifecycle.Exited -> exitCode
        }

        override fun destroy() {
            destroyCalls += 1
        }

        override fun destroyForcibly(): Process {
            forceDestroyCalls += 1
            when (behavior) {
                FakeProcessBehavior.NeverReaps -> Unit
                is FakeProcessBehavior.Exit,
                FakeProcessBehavior.Running,
                FakeProcessBehavior.TimeoutThenForceReap -> lifecycle = Lifecycle.Exited
            }
            return this
        }

        override fun isAlive(): Boolean = lifecycle == Lifecycle.Running

        fun reapExternally() {
            lifecycle = Lifecycle.Exited
        }
    }
}

private object FixedChildInspector : AndroidChildProcessInspector {
    override fun inspect(child: AndroidChildProcessIdentityWire): AndroidChildProcessInspection =
        AndroidChildProcessInspection.ExactChildRunning

    override fun capture(pid: Long): AndroidChildProcessCapture =
        AndroidChildProcessCapture.Identified(
            AndroidChildProcessIdentityWire(pid, BigInteger("987654")),
        )

    override fun snapshotCurrentThreadChildren(): AndroidCurrentThreadChildSnapshot =
        AndroidCurrentThreadChildSnapshot.Captured(101L, 202L, emptySet())

    override fun captureStartedChild(
        snapshot: AndroidCurrentThreadChildSnapshot,
    ): AndroidChildProcessCapture =
        AndroidChildProcessCapture.Identified(
            AndroidChildProcessIdentityWire(321L, BigInteger("987654")),
        )
}

private fun startContext(generation: Long = 1L, token: BigInteger = BigInteger("42")) =
    CoreProcessStartContextWire(
        CoreIdentityWire("mihomo", "android-active", null),
        BigInteger.valueOf(generation),
        token,
    )

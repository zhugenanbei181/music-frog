package com.musicfrog.infiltrator

import java.io.File
import java.math.BigInteger
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class CoreLaunchSpecWireTest {
    @Test
    fun decodesRustOwnerConfigurationIdentityBinaryWorkingDirectoryAndArguments() {
        val decoded = CoreStartRequestWire.decode(validRequest())

        assertEquals(
            CoreStartRequestDecode.Accepted(
                CoreStartRequestWire(
                    CoreLaunchSpecWire(
                        CoreLaunchConfigurationIdentityWire("mihomo", "primary", "sha256:abc123"),
                        CoreLaunchBinaryWire.MihomoBundled,
                        "/data/app/files",
                        CoreLaunchArgumentsWire.Mihomo(
                            "/data/app/files/configs/engines/mihomo/primary.yaml",
                        ),
                    ),
                    CoreProcessStartContextWire(
                        CoreIdentityWire("mihomo", "android-active", null),
                        BigInteger("7"),
                        BigInteger("42"),
                    ),
                ),
            ),
            decoded,
        )
    }

    @Test
    fun rejectsUnsupportedOrMismatchedEngineMissingIdentityAndUnknownFields() {
        val unsupportedEngine = validRequest().replace("mihomo", "unknown-engine")
        val mismatchedBinaryEngine = validRequest().replace("mihomo", "sing-box")
        val missingIdentity = validRequest().replace("\"profile_id\":\"primary\",", "")
        val unknownField = validRequest().replace(
            "\"profile_id\":\"primary\",",
            "\"profile_id\":\"primary\",\"legacy_config_name\":\"default.yaml\",",
        )

        listOf(unsupportedEngine, mismatchedBinaryEngine, missingIdentity, unknownField).forEach { json ->
            assertTrue(CoreStartRequestWire.decode(json) is CoreStartRequestDecode.Rejected)
        }

        val standaloneContext = validRequest().replace("\"Managed\":", "\"Standalone\":")
        assertTrue(CoreStartRequestWire.decode(standaloneContext) is CoreStartRequestDecode.Rejected)
        val zeroSession = validRequest().replace("\"session_token\":42", "\"session_token\":0")
        assertTrue(CoreStartRequestWire.decode(zeroSession) is CoreStartRequestDecode.Rejected)
        val zeroGeneration = validRequest().replace("\"generation\":7", "\"generation\":0")
        assertTrue(CoreStartRequestWire.decode(zeroGeneration) is CoreStartRequestDecode.Rejected)
        val wrongInstance = validRequest().replace("android-active", "desktop-active")
        assertTrue(CoreStartRequestWire.decode(wrongInstance) is CoreStartRequestDecode.Rejected)
    }

    @Test
    fun decodesManagedSingBoxBinaryAndRequiresItToMatchTheOwnerEngine() {
        val request = validRequest()
            .replace("\"MihomoBundled\"", "{\"SingBoxManaged\":{\"version\":\"v1.14.3\",\"target\":\"arm64_v8a\"}}")
            .replace(
                "\"Mihomo\":{\"configuration_path\":\"/data/app/files/configs/engines/mihomo/primary.yaml\"}",
                "\"SingBox\":{\"configuration_path\":\"/data/app/files/configs/engines/sing-box/primary.json\"}",
            )
            .replace("\"engine_id\":\"mihomo\"", "\"engine_id\":\"sing-box\"")
            .replace("\"engine\":\"mihomo\"", "\"engine\":\"sing-box\"")
        val decoded = CoreStartRequestWire.decode(request) as CoreStartRequestDecode.Accepted

        assertEquals(
            CoreLaunchBinaryWire.SingBoxManaged(
                "v1.14.3",
                AndroidCoreBinaryTargetWire.Arm64V8a,
            ),
            decoded.request.launchSpec.binary,
        )
        assertEquals(
            CoreLaunchArgumentsWire.SingBox(
                "/data/app/files/configs/engines/sing-box/primary.json",
            ),
            decoded.request.launchSpec.launchArguments,
        )
        assertEquals("aarch64-linux-android", AndroidCoreBinaryTargetWire.Arm64V8a.targetTriple)

        val mismatchedOwner = request.replace("\"engine\":\"sing-box\"", "\"engine\":\"mihomo\"")
        assertTrue(CoreStartRequestWire.decode(mismatchedOwner) is CoreStartRequestDecode.Rejected)
    }

    @Test
    fun rejectsRelativeWorkingDirectoryAndInvalidTypedLaunchArguments() {
        val relativeWorkingDirectory = validRequest().replace("/data/app/files", "files")
        val emptyConfigurationPath = validRequest().replace(
            "/data/app/files/configs/engines/mihomo/primary.yaml",
            " ",
        )
        val wrongEngineArguments = validRequest().replace(
            "\"Mihomo\":{\"configuration_path\":",
            "\"SingBox\":{\"configuration_path\":",
        )
        val unknownArgumentsField = validRequest().replace(
            "\"configuration_path\":\"/data/app/files/configs/engines/mihomo/primary.yaml\"",
            "\"configuration_path\":\"/data/app/files/configs/engines/mihomo/primary.yaml\",\"legacy\":true",
        )

        listOf(
            relativeWorkingDirectory,
            emptyConfigurationPath,
            wrongEngineArguments,
            unknownArgumentsField,
        ).forEach { json ->
            assertTrue(CoreStartRequestWire.decode(json) is CoreStartRequestDecode.Rejected)
        }
    }

    @Test
    fun spawnCommandUsesTheRustSelectedWorkingDirectoryAndArgumentVector() {
        val accepted = CoreStartRequestWire.decode(validRequest()) as CoreStartRequestDecode.Accepted
        val command = CoreProcessSpawnCommand.from(File("/native/libmihomo.so"), accepted.request.launchSpec)
        val builder = command.processBuilder()

        assertEquals(
            listOf(
                "/native/libmihomo.so",
                "-d",
                "/data/app/files",
                "-f",
                "/data/app/files/configs/engines/mihomo/primary.yaml",
            ),
            builder.command(),
        )
        assertEquals(File("/data/app/files"), builder.directory())
    }

    private fun validRequest(): String =
        """{"launch_spec":{"configuration_identity":{"engine_id":"mihomo","profile_id":"primary","document_revision":"sha256:abc123"},"binary":"MihomoBundled","working_directory":"/data/app/files","launch_arguments":{"Mihomo":{"configuration_path":"/data/app/files/configs/engines/mihomo/primary.yaml"}}},"start_context":{"Managed":{"identity":{"engine":"mihomo","instance":"android-active","version":null},"generation":7,"session_token":42}}}"""
}

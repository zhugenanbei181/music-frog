package com.musicfrog.infiltrator

import java.io.File
import java.math.BigInteger
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class CoreLaunchSpecWireTest {
    @Test
    fun decodesRustOwnerConfigurationIdentityWorkingDirectoryAndArguments() {
        val decoded = CoreStartRequestWire.decode(validRequest())

        assertEquals(
            CoreStartRequestDecode.Accepted(
                CoreStartRequestWire(
                    CoreLaunchSpecWire(
                        CoreLaunchConfigurationIdentityWire("mihomo", "primary", "sha256:abc123"),
                        "/data/app/files",
                        listOf(
                            "-d",
                            "/data/app/files",
                            "-f",
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
    fun rejectsUnsupportedEngineMissingIdentityAndUnknownConfigurationFields() {
        val unsupportedEngine = validRequest().replace("mihomo", "sing-box")
        val missingIdentity = validRequest().replace("\"profile_id\": \"primary\",", "")
        val unknownField = validRequest().replace("\"profile_id\": \"primary\",", "\"profile_id\": \"primary\",\n    \"legacy_config_name\": \"default.yaml\",")

        listOf(unsupportedEngine, missingIdentity, unknownField).forEach { json ->
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
    fun rejectsRelativeWorkingDirectoryAndEmptyLaunchArguments() {
        val relativeWorkingDirectory = validRequest().replace("/data/app/files", "files")
        val emptyArguments = validRequest().replace(
            "[\"-d\", \"/data/app/files\", \"-f\", \"/data/app/files/configs/engines/mihomo/primary.yaml\"]",
            "[]",
        )
        val nonStringArgument = validRequest().replace("\"-d\"", "7")

        listOf(relativeWorkingDirectory, emptyArguments, nonStringArgument).forEach { json ->
            assertTrue(CoreStartRequestWire.decode(json) is CoreStartRequestDecode.Rejected)
        }
    }

    @Test
    fun spawnCommandUsesTheRustSelectedWorkingDirectoryAndArgumentVector() {
        val accepted = CoreStartRequestWire.decode(validRequest()) as CoreStartRequestDecode.Accepted
        val command = MihomoSpawnCommand.from(File("/native/libmihomo.so"), accepted.request.launchSpec)
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
        """{"launch_spec":{"configuration_identity":{"engine_id":"mihomo","profile_id":"primary","document_revision":"sha256:abc123"},"working_directory":"/data/app/files","launch_arguments":["-d","/data/app/files","-f","/data/app/files/configs/engines/mihomo/primary.yaml"]},"start_context":{"Managed":{"identity":{"engine":"mihomo","instance":"android-active","version":null},"generation":7,"session_token":42}}}"""
}

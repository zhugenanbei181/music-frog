package com.musicfrog.infiltrator

import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class CoreLaunchSpecWireTest {
    @Test
    fun decodesRustOwnerConfigurationIdentityWorkingDirectoryAndArguments() {
        val decoded = CoreLaunchSpecWire.decode(
            """
            {
              "configuration_identity": {
                "engine_id": "mihomo",
                "profile_id": "primary",
                "document_revision": "sha256:abc123"
              },
              "working_directory": "/data/app/files",
              "launch_arguments": [
                "-d",
                "/data/app/files",
                "-f",
                "/data/app/files/configs/engines/mihomo/primary.yaml"
              ]
            }
            """.trimIndent(),
        )

        assertEquals(
            CoreLaunchSpecDecode.Accepted(
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
            ),
            decoded,
        )
    }

    @Test
    fun rejectsUnsupportedEngineMissingIdentityAndUnknownConfigurationFields() {
        val unsupportedEngine = validSpec().replace("mihomo", "sing-box")
        val missingIdentity = validSpec().replace("\"profile_id\": \"primary\",", "")
        val unknownField = validSpec().replace("\"profile_id\": \"primary\",", "\"profile_id\": \"primary\",\n    \"legacy_config_name\": \"default.yaml\",")

        listOf(unsupportedEngine, missingIdentity, unknownField).forEach { json ->
            assertTrue(CoreLaunchSpecWire.decode(json) is CoreLaunchSpecDecode.Rejected)
        }
    }

    @Test
    fun rejectsRelativeWorkingDirectoryAndEmptyLaunchArguments() {
        val relativeWorkingDirectory = validSpec().replace("/data/app/files", "files")
        val emptyArguments = validSpec().replace(
            "[\"-d\", \"/data/app/files\", \"-f\", \"/data/app/files/configs/engines/mihomo/primary.yaml\"]",
            "[]",
        )
        val nonStringArgument = validSpec().replace("\"-d\"", "7")

        listOf(relativeWorkingDirectory, emptyArguments, nonStringArgument).forEach { json ->
            assertTrue(CoreLaunchSpecWire.decode(json) is CoreLaunchSpecDecode.Rejected)
        }
    }

    @Test
    fun spawnCommandUsesTheRustSelectedWorkingDirectoryAndArgumentVector() {
        val accepted = CoreLaunchSpecWire.decode(validSpec()) as CoreLaunchSpecDecode.Accepted
        val command = MihomoSpawnCommand.from(File("/native/libmihomo.so"), accepted.spec)
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

    private fun validSpec(): String =
        """{"configuration_identity":{"engine_id":"mihomo","profile_id":"primary","document_revision":"sha256:abc123"},"working_directory":"/data/app/files","launch_arguments":["-d","/data/app/files","-f","/data/app/files/configs/engines/mihomo/primary.yaml"]}"""
}

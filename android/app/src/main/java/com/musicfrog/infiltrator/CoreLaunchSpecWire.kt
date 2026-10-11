package com.musicfrog.infiltrator

import org.json.JSONObject

internal data class CoreLaunchConfigurationIdentityWire(
    val engineId: String,
    val profileId: String,
    val documentRevision: String,
)

internal data class CoreLaunchSpecWire(
    val configurationIdentity: CoreLaunchConfigurationIdentityWire,
    val workingDirectory: String,
    val launchArguments: List<String>,
) {
    companion object {
        fun decode(raw: String): CoreLaunchSpecDecode = runCatching {
            val json = JSONObject(raw)
            require(json.keys().asSequence().toSet() == setOf(
                "configuration_identity", "working_directory", "launch_arguments",
            ))
            val identity = json.getJSONObject("configuration_identity")
            require(identity.keys().asSequence().toSet() == setOf(
                "engine_id", "profile_id", "document_revision",
            ))
            val configurationIdentity = CoreLaunchConfigurationIdentityWire(
                engineId = identity.requiredString("engine_id").also { require(it == "mihomo") },
                profileId = identity.requiredString("profile_id").also { require(it.isNotBlank()) },
                documentRevision = identity.requiredString("document_revision").also {
                    require(it.isNotBlank())
                },
            )
            val workingDirectory = json.requiredString("working_directory").also {
                require(it.isNotBlank() && java.io.File(it).isAbsolute)
            }
            val argumentsJson = json.getJSONArray("launch_arguments")
            val arguments = buildList(argumentsJson.length()) {
                for (index in 0 until argumentsJson.length()) {
                    add(
                        (argumentsJson.get(index) as? String)
                            ?.also { require(it.isNotBlank()) }
                            ?: error("launch argument at index $index must be a string"),
                    )
                }
            }
            require(arguments.isNotEmpty())
            CoreLaunchSpecDecode.Accepted(
                CoreLaunchSpecWire(configurationIdentity, workingDirectory, arguments),
            )
        }.getOrElse { error ->
            CoreLaunchSpecDecode.Rejected(error.message ?: "invalid Core launch spec")
        }
    }
}

private fun JSONObject.requiredString(key: String): String =
    get(key) as? String ?: error("$key must be a string")

internal sealed interface CoreLaunchSpecDecode {
    data class Accepted(val spec: CoreLaunchSpecWire) : CoreLaunchSpecDecode

    data class Rejected(val reason: String) : CoreLaunchSpecDecode
}

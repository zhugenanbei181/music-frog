package com.musicfrog.infiltrator

import org.json.JSONObject
import java.math.BigInteger

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
        internal fun decodeLaunchSpec(raw: String): CoreLaunchSpecDecode = runCatching {
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
            CoreLaunchSpecDecode.Accepted(CoreLaunchSpecWire(configurationIdentity, workingDirectory, arguments))
        }.getOrElse { error ->
            CoreLaunchSpecDecode.Rejected(error.message ?: "invalid Core launch spec")
        }
    }
}

internal data class CoreProcessStartContextWire(
    val identity: CoreIdentityWire,
    val generation: BigInteger,
    val sessionToken: BigInteger,
) {
    fun toManagedJson(): JSONObject {
        val identityJson = JSONObject()
            .put("engine", identity.engine)
            .put("instance", identity.instance)
            .put("version", identity.version ?: JSONObject.NULL)
        val contextJson = JSONObject()
            .put("identity", identityJson)
            .put("generation", generation)
            .put("session_token", sessionToken)
        return JSONObject().put("Managed", contextJson)
    }

    companion object {
        fun decode(raw: String): CoreProcessStartContextDecode = runCatching {
            val taggedContext = JSONObject(raw)
            require(taggedContext.keys().asSequence().toSet() == setOf("Managed")) {
                "Android Core start requires a managed owner context"
            }
            val context = taggedContext.getJSONObject("Managed")
            require(context.keys().asSequence().toSet() == setOf("identity", "generation", "session_token"))
            val identityJson = context.getJSONObject("identity")
            require(identityJson.keys().asSequence().toSet() == setOf("engine", "instance", "version"))
            val identity = CoreIdentityWire(
                engine = identityJson.requiredString("engine").also { require(it == "mihomo") },
                instance = identityJson.requiredString("instance").also {
                    require(it == "android-active")
                },
                version = if (identityJson.isNull("version")) null else identityJson.requiredString("version"),
            )
            val generation = BigInteger(context.get("generation").toString())
                .also { require(it.signum() > 0) }
            val sessionToken = BigInteger(context.get("session_token").toString())
                .also { require(it.signum() > 0) }
            CoreProcessStartContextDecode.Accepted(
                CoreProcessStartContextWire(identity, generation, sessionToken),
            )
        }.getOrElse { error ->
            CoreProcessStartContextDecode.Rejected(error.message ?: "invalid Core owner context")
        }
    }
}

internal data class CoreIdentityWire(
    val engine: String,
    val instance: String,
    val version: String?,
)

internal data class CoreStartRequestWire(
    val launchSpec: CoreLaunchSpecWire,
    val context: CoreProcessStartContextWire,
) {
    companion object {
        fun decode(raw: String): CoreStartRequestDecode = runCatching {
            val request = JSONObject(raw)
            require(request.keys().asSequence().toSet() == setOf("launch_spec", "start_context"))
            val decodedLaunchSpec = CoreLaunchSpecWire.decodeLaunchSpec(
                request.getJSONObject("launch_spec").toString(),
            )
            val launchSpec = when (decodedLaunchSpec) {
                is CoreLaunchSpecDecode.Accepted -> decodedLaunchSpec.spec
                is CoreLaunchSpecDecode.Rejected -> error(decodedLaunchSpec.reason)
            }
            val decodedStartContext = CoreProcessStartContextWire.decode(
                request.getJSONObject("start_context").toString(),
            )
            val startContext = when (decodedStartContext) {
                is CoreProcessStartContextDecode.Accepted -> decodedStartContext.context
                is CoreProcessStartContextDecode.Rejected -> error(decodedStartContext.reason)
            }
            CoreStartRequestDecode.Accepted(
                CoreStartRequestWire(
                    launchSpec,
                    startContext,
                ),
            )
        }.getOrElse { error ->
            CoreStartRequestDecode.Rejected(error.message ?: "invalid Core start request")
        }
    }
}

private fun JSONObject.requiredString(key: String): String =
    get(key) as? String ?: error("$key must be a string")

internal sealed interface CoreLaunchSpecDecode {
    data class Accepted(val spec: CoreLaunchSpecWire) : CoreLaunchSpecDecode

    data class Rejected(val reason: String) : CoreLaunchSpecDecode
}

internal sealed interface CoreStartRequestDecode {
    data class Accepted(val request: CoreStartRequestWire) : CoreStartRequestDecode

    data class Rejected(val reason: String) : CoreStartRequestDecode
}

internal sealed interface CoreProcessStartContextDecode {
    data class Accepted(val context: CoreProcessStartContextWire) : CoreProcessStartContextDecode

    data class Rejected(val reason: String) : CoreProcessStartContextDecode
}

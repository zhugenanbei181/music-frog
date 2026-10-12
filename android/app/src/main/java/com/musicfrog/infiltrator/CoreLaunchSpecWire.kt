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
    val binary: CoreLaunchBinaryWire,
    val workingDirectory: String,
    val launchArguments: CoreLaunchArgumentsWire,
) {
    companion object {
        internal fun decodeLaunchSpec(raw: String): CoreLaunchSpecDecode = runCatching {
            val json = JSONObject(raw)
            require(json.keys().asSequence().toSet() == setOf(
                "configuration_identity", "binary", "working_directory", "launch_arguments",
            ))
            val identity = json.getJSONObject("configuration_identity")
            require(identity.keys().asSequence().toSet() == setOf(
                "engine_id", "profile_id", "document_revision",
            ))
            val configurationIdentity = CoreLaunchConfigurationIdentityWire(
                engineId = identity.requiredString("engine_id").also { CoreEngineIdWire.decode(it) },
                profileId = identity.requiredString("profile_id").also { require(it.isNotBlank()) },
                documentRevision = identity.requiredString("document_revision").also {
                    require(it.isNotBlank())
                },
            )
            val binary = CoreLaunchBinaryWire.decode(json.get("binary"))
            require(binary.engineId.wireValue == configurationIdentity.engineId) {
                "Core launch binary does not match its engine identity"
            }
            val workingDirectory = json.requiredString("working_directory").also {
                require(it.isNotBlank() && java.io.File(it).isAbsolute)
            }
            val launchArguments = CoreLaunchArgumentsWire.decode(json.get("launch_arguments"))
            require(launchArguments.engineId.wireValue == configurationIdentity.engineId) {
                "Core launch arguments do not match their engine identity"
            }
            CoreLaunchSpecDecode.Accepted(
                CoreLaunchSpecWire(configurationIdentity, binary, workingDirectory, launchArguments),
            )
        }.getOrElse { error ->
            CoreLaunchSpecDecode.Rejected(error.message ?: "invalid Core launch spec")
        }
    }
}

internal sealed interface CoreLaunchArgumentsWire {
    val engineId: CoreEngineIdWire
    val configurationPath: String

    data class Mihomo(override val configurationPath: String) : CoreLaunchArgumentsWire {
        override val engineId: CoreEngineIdWire = CoreEngineIdWire.Mihomo
    }

    data class SingBox(override val configurationPath: String) : CoreLaunchArgumentsWire {
        override val engineId: CoreEngineIdWire = CoreEngineIdWire.SingBox
    }

    companion object {
        fun decode(value: Any): CoreLaunchArgumentsWire {
            require(value is JSONObject) { "Core launch arguments must be a typed object" }
            require(value.length() == 1) { "Core launch arguments require exactly one engine variant" }
            val variant = value.keys().next()
            val fields = value.getJSONObject(variant)
            require(fields.keys().asSequence().toSet() == setOf("configuration_path"))
            val path = fields.requiredString("configuration_path")
            require(path.isNotBlank() && '\u0000' !in path && java.io.File(path).isAbsolute)
            return when (variant) {
                "Mihomo" -> Mihomo(path)
                "SingBox" -> SingBox(path)
                else -> error("unsupported Core launch argument variant")
            }
        }
    }
}

internal enum class CoreEngineIdWire(val wireValue: String) {
    Mihomo("mihomo"),
    SingBox("sing-box");

    companion object {
        fun decode(value: String): CoreEngineIdWire = entries.firstOrNull { it.wireValue == value }
            ?: error("unsupported Core engine identity")
    }
}

internal enum class AndroidCoreBinaryTargetWire(val wireValue: String, val targetTriple: String) {
    Arm64V8a("arm64_v8a", "aarch64-linux-android"),
    X86_64("x86_64", "x86_64-linux-android");

    companion object {
        fun decode(value: String): AndroidCoreBinaryTargetWire = entries.firstOrNull {
            it.wireValue == value
        } ?: error("unsupported Android Core binary target")
    }
}

internal sealed interface CoreLaunchBinaryWire {
    val engineId: CoreEngineIdWire

    data object MihomoBundled : CoreLaunchBinaryWire {
        override val engineId: CoreEngineIdWire = CoreEngineIdWire.Mihomo
    }

    data class SingBoxManaged(
        val version: String,
        val target: AndroidCoreBinaryTargetWire,
    ) : CoreLaunchBinaryWire {
        override val engineId: CoreEngineIdWire = CoreEngineIdWire.SingBox
    }

    companion object {
        fun decode(value: Any): CoreLaunchBinaryWire = when (value) {
            is String -> when (value) {
                "MihomoBundled" -> MihomoBundled
                else -> error("unsupported Core launch binary")
            }
            is JSONObject -> {
                require(value.keys().asSequence().toSet() == setOf("SingBoxManaged"))
                val managed = value.getJSONObject("SingBoxManaged")
                require(managed.keys().asSequence().toSet() == setOf("version", "target"))
                SingBoxManaged(
                    version = managed.requiredString("version").also { require(it.isNotBlank()) },
                    target = AndroidCoreBinaryTargetWire.decode(managed.requiredString("target")),
                )
            }
            else -> error("Core launch binary has an invalid wire representation")
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
                engine = identityJson.requiredString("engine").also { CoreEngineIdWire.decode(it) },
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
            require(launchSpec.configurationIdentity.engineId == startContext.identity.engine) {
                "Core launch specification and owner context belong to different engines"
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

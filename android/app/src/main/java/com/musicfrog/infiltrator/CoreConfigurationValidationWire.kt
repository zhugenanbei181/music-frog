package com.musicfrog.infiltrator

import org.json.JSONObject
import java.io.File

internal class CoreConfigurationDigestWire private constructor(internal val sha256Hex: String) {
    fun encoded(): String = "sha256:$sha256Hex"

    override fun equals(other: Any?): Boolean = other is CoreConfigurationDigestWire && sha256Hex == other.sha256Hex

    override fun hashCode(): Int = sha256Hex.hashCode()

    companion object {
        private val SHA256 = Regex("[0-9a-f]{64}")

        fun decode(raw: String): CoreConfigurationDigestWireDecode = when {
            !raw.startsWith("sha256:") -> CoreConfigurationDigestWireDecode.Rejected

            !SHA256.matches(raw.removePrefix("sha256:")) ->
                CoreConfigurationDigestWireDecode.Rejected

            else -> CoreConfigurationDigestWireDecode.Accepted(
                CoreConfigurationDigestWire(raw.removePrefix("sha256:")),
            )
        }
    }
}

internal sealed interface CoreConfigurationDigestWireDecode {
    data class Accepted(val digest: CoreConfigurationDigestWire) : CoreConfigurationDigestWireDecode
    data object Rejected : CoreConfigurationDigestWireDecode
}

internal data class CoreConfigurationValidationRequestWire(
    val requestId: Long,
    val timeoutMillis: Long,
    val configurationIdentity: CoreLaunchConfigurationIdentityWire,
    val candidatePath: String,
    val candidateDigest: CoreConfigurationDigestWire,
    val workingDirectory: String,
) {
    fun toJson(): JSONObject = JSONObject()
        .put("request_id", requestId)
        .put("timeout_millis", timeoutMillis)
        .put("configuration_identity", configurationIdentity.toJson())
        .put("candidate_path", candidatePath)
        .put("candidate_digest", candidateDigest.encoded())
        .put("working_directory", workingDirectory)

    companion object {
        fun decode(raw: String): CoreConfigurationValidationRequestDecode = runCatching {
            val json = JSONObject(raw)
            require(
                json.keys().asSequence().toSet() == setOf(
                    "request_id",
                    "timeout_millis",
                    "configuration_identity",
                    "candidate_path",
                    "candidate_digest",
                    "working_directory",
                ),
            )
            val identityJson = json.getJSONObject("configuration_identity")
            require(
                identityJson.keys().asSequence().toSet() == setOf(
                    "engine_id",
                    "profile_id",
                    "document_revision",
                ),
            )
            val profileId = identityJson.requiredString("profile_id").also {
                require(it.isNotBlank() && it.trim() == it && it.length <= 128)
                require(it != "." && it != ".." && ".." !in it)
                require(it.none { character -> character in "/\\:" || character.isISOControl() })
                require(!it.endsWith('.') && !it.endsWith(' '))
            }
            val identity = CoreLaunchConfigurationIdentityWire(
                engineId = identityJson.requiredString("engine_id").also { require(it == "mihomo") },
                profileId = profileId,
                documentRevision = identityJson.requiredString("document_revision")
                    .also { require(it.isNotBlank()) },
            )
            val candidatePath = json.requiredString("candidate_path")
                .also { require(it.isNotBlank() && File(it).isAbsolute && '\u0000' !in it) }
            val workingDirectory = json.requiredString("working_directory")
                .also { require(it.isNotBlank() && File(it).isAbsolute && '\u0000' !in it) }
            val digest = when (
                val decoded = CoreConfigurationDigestWire.decode(
                    json.requiredString("candidate_digest"),
                )
            ) {
                is CoreConfigurationDigestWireDecode.Accepted -> decoded.digest
                CoreConfigurationDigestWireDecode.Rejected -> error("candidate digest is not SHA-256")
            }
            CoreConfigurationValidationRequestWire(
                requestId = json.requiredLong("request_id").also { require(it > 0L) },
                timeoutMillis = json.requiredLong("timeout_millis").also { require(it > 0L) },
                configurationIdentity = identity,
                candidatePath = candidatePath,
                candidateDigest = digest,
                workingDirectory = workingDirectory,
            )
        }.fold(
            onSuccess = CoreConfigurationValidationRequestDecode::Accepted,
            onFailure = { error ->
                CoreConfigurationValidationRequestDecode.InvalidRequest(
                    error.message ?: "invalid Core configuration validation request",
                )
            },
        )
    }
}

internal sealed interface CoreConfigurationValidationRequestDecode {
    data class Accepted(val request: CoreConfigurationValidationRequestWire) : CoreConfigurationValidationRequestDecode

    data class InvalidRequest(val reason: String) : CoreConfigurationValidationRequestDecode
}

private fun CoreLaunchConfigurationIdentityWire.toJson(): JSONObject = JSONObject()
    .put("engine_id", engineId)
    .put("profile_id", profileId)
    .put("document_revision", documentRevision)

private fun JSONObject.requiredString(key: String): String = get(key) as? String ?: error("$key must be a string")

private fun JSONObject.requiredLong(key: String): Long {
    val number = get(key) as? Number ?: error("$key must be an integer")
    return number.toString().toLongOrNull() ?: error("$key must be an integer")
}

internal sealed interface CoreConfigurationValidationOutcomeWire {
    data object Validated : CoreConfigurationValidationOutcomeWire
    data class ConfigurationRejected(val exitCode: Int) : CoreConfigurationValidationOutcomeWire
    data object CoreActive : CoreConfigurationValidationOutcomeWire
    data object BinaryUnavailable : CoreConfigurationValidationOutcomeWire
    data object CandidateMissing : CoreConfigurationValidationOutcomeWire
    data object CandidatePathRejected : CoreConfigurationValidationOutcomeWire
    data object CandidateDigestMismatch : CoreConfigurationValidationOutcomeWire
    data object CandidateIdentityMismatch : CoreConfigurationValidationOutcomeWire
    data object DeadlineExpired : CoreConfigurationValidationOutcomeWire
    data object DeadlineTooFar : CoreConfigurationValidationOutcomeWire
    data object TimedOut : CoreConfigurationValidationOutcomeWire
    data object TimeoutCleanupFailed : CoreConfigurationValidationOutcomeWire
    data object Interrupted : CoreConfigurationValidationOutcomeWire
    data object InterruptedCleanupFailed : CoreConfigurationValidationOutcomeWire
    data object ProcessCleanupFailed : CoreConfigurationValidationOutcomeWire
    data object ValidationCleanupPending : CoreConfigurationValidationOutcomeWire
    data object CoreProcessObservationUnavailable : CoreConfigurationValidationOutcomeWire
    data object ProcessLaunchFailed : CoreConfigurationValidationOutcomeWire
    data object NotVpnOwner : CoreConfigurationValidationOutcomeWire
}

internal sealed interface CoreConfigurationValidationReplyWire {
    data class Receipt(val request: CoreConfigurationValidationRequestWire, val outcome: CoreConfigurationValidationOutcomeWire) :
        CoreConfigurationValidationReplyWire

    data class InvalidRequest(val reason: String) : CoreConfigurationValidationReplyWire
}

internal fun CoreConfigurationValidationReplyWire.toJson(): String = when (this) {
    is CoreConfigurationValidationReplyWire.Receipt -> JSONObject()
        .put(
            "Receipt",
            JSONObject()
                .put("request", request.toJson())
                .put("outcome", outcome.toJsonValue()),
        )
        .toString()

    is CoreConfigurationValidationReplyWire.InvalidRequest -> JSONObject()
        .put("InvalidRequest", JSONObject().put("reason", reason))
        .toString()
}

private fun CoreConfigurationValidationOutcomeWire.toJsonValue(): Any = when (this) {
    CoreConfigurationValidationOutcomeWire.Validated -> "Validated"

    is CoreConfigurationValidationOutcomeWire.ConfigurationRejected -> JSONObject()
        .put("ConfigurationRejected", JSONObject().put("exit_code", exitCode))

    CoreConfigurationValidationOutcomeWire.CoreActive -> "CoreActive"

    CoreConfigurationValidationOutcomeWire.BinaryUnavailable -> "BinaryUnavailable"

    CoreConfigurationValidationOutcomeWire.CandidateMissing -> "CandidateMissing"

    CoreConfigurationValidationOutcomeWire.CandidatePathRejected -> "CandidatePathRejected"

    CoreConfigurationValidationOutcomeWire.CandidateDigestMismatch -> "CandidateDigestMismatch"

    CoreConfigurationValidationOutcomeWire.CandidateIdentityMismatch -> "CandidateIdentityMismatch"

    CoreConfigurationValidationOutcomeWire.DeadlineExpired -> "DeadlineExpired"

    CoreConfigurationValidationOutcomeWire.DeadlineTooFar -> "DeadlineTooFar"

    CoreConfigurationValidationOutcomeWire.TimedOut -> "TimedOut"

    CoreConfigurationValidationOutcomeWire.TimeoutCleanupFailed -> "TimeoutCleanupFailed"

    CoreConfigurationValidationOutcomeWire.Interrupted -> "Interrupted"

    CoreConfigurationValidationOutcomeWire.InterruptedCleanupFailed -> "InterruptedCleanupFailed"

    CoreConfigurationValidationOutcomeWire.ProcessCleanupFailed -> "ProcessCleanupFailed"

    CoreConfigurationValidationOutcomeWire.ValidationCleanupPending -> "ValidationCleanupPending"

    CoreConfigurationValidationOutcomeWire.CoreProcessObservationUnavailable -> "CoreProcessObservationUnavailable"

    CoreConfigurationValidationOutcomeWire.ProcessLaunchFailed -> "ProcessLaunchFailed"

    CoreConfigurationValidationOutcomeWire.NotVpnOwner -> "NotVpnOwner"
}

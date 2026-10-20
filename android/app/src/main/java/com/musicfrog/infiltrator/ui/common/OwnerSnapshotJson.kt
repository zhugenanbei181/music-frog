package com.musicfrog.infiltrator.ui.common

import org.json.JSONArray
import org.json.JSONObject
import java.math.BigInteger
import kotlin.math.roundToLong

internal fun parseCoreScopeObservation(scopeJson: JSONObject, rawScopeJson: String): CoreScopeObservation {
    val identityJson = scopeJson.getJSONObject("identity")
    val rawGeneration = rawJsonValueAtPath(rawScopeJson, "generation")
        ?: throw IllegalArgumentException("Owner CoreScope has no generation")
    val rawSessionToken = rawJsonValueAtPath(rawScopeJson, "session_token")
        ?: throw IllegalArgumentException("Owner CoreScope has no session token")
    return CoreScopeObservation(
        identity = CoreIdentityObservation(
            engineId = identityJson.getString("engine"),
            instanceId = identityJson.getString("instance"),
            version = identityJson.nullableString("version"),
        ),
        generation = parseUnsignedDecimal(rawGeneration, "generation"),
        sessionToken = parseNullableUnsignedDecimal(rawSessionToken, "session_token"),
        json = rawScopeJson,
    )
}

/** Preserve the CoreScope fields from a CoreSnapshot without carrying its read-only fields. */
internal fun coreScopeJsonFromSnapshot(rawCoreJson: String): String {
    val identity = rawJsonValueAtPath(rawCoreJson, "identity")
        ?.takeUnless { it == "null" }
        ?: throw IllegalArgumentException("Owner CoreSnapshot has no Core identity")
    val generation = rawJsonValueAtPath(rawCoreJson, "generation")
        ?: throw IllegalArgumentException("Owner CoreSnapshot has no generation")
    val sessionToken = rawJsonValueAtPath(rawCoreJson, "session_token")
        ?: throw IllegalArgumentException("Owner CoreSnapshot has no session token")
    return """{"identity":$identity,"generation":$generation,"session_token":$sessionToken}"""
}

internal fun parseOwnerFailure(value: JSONObject): OwnerFailure = OwnerFailure(
    code = value.getString("code").toOwnerErrorCode(),
    message = value.getString("message"),
    retryable = value.getBoolean("retryable"),
    reason = parseOwnerFailureReason(value.opt("reason")),
)

internal fun parseOwnerFailureReason(value: Any?): OwnerFailureReason? {
    if (value == null || value === JSONObject.NULL) return null
    if (value is String) {
        return when (value) {
            "MixinYaml" -> OwnerFailureReason.MixinYaml
            "ActiveProfileInconsistent" -> OwnerFailureReason.ActiveProfileInconsistent
            "QuotaSourceChanged" -> OwnerFailureReason.QuotaSourceChanged
            "CurrentTimeUnavailable" -> OwnerFailureReason.CurrentTimeUnavailable
            "DownloadCanceled" -> OwnerFailureReason.DownloadCanceled
            else -> throw IllegalArgumentException("Unknown owner failure reason: $value")
        }
    }
    val reason = value as? JSONObject
        ?: throw IllegalArgumentException("Owner failure reason is malformed")
    val variant = reason.singleVariant()
    return when (variant) {
        "YamlSyntax" -> reason.getJSONObject(variant).let { payload ->
            OwnerFailureReason.YamlSyntax(
                line = payload.unsigned("line"),
                column = payload.unsigned("column"),
            )
        }

        "MixinYaml" -> reason.unitReason(variant, OwnerFailureReason.MixinYaml)

        "ActiveProfileInconsistent" -> reason.unitReason(variant, OwnerFailureReason.ActiveProfileInconsistent)

        "QuotaSourceChanged" -> reason.unitReason(variant, OwnerFailureReason.QuotaSourceChanged)

        "CurrentTimeUnavailable" -> reason.unitReason(variant, OwnerFailureReason.CurrentTimeUnavailable)

        "DownloadCanceled" -> reason.unitReason(variant, OwnerFailureReason.DownloadCanceled)

        else -> throw IllegalArgumentException("Unknown owner failure reason: $variant")
    }
}

private fun <T : OwnerFailureReason> JSONObject.unitReason(name: String, reason: T): T {
    val payload = opt(name)
    require(payload == null || payload === JSONObject.NULL) {
        "Owner unit failure reason $name must not have payload"
    }
    return reason
}

private fun String.toOwnerErrorCode(): OwnerErrorCode = try {
    OwnerErrorCode.valueOf(this)
} catch (error: IllegalArgumentException) {
    throw IllegalArgumentException("Unknown owner error code: $this", error)
}

internal fun parseUnsignedDecimal(rawValue: String, name: String): String {
    val value = rawValue.trim()
    if (!value.matches(Regex("0|[1-9][0-9]*")) || value.toULongOrNull() == null) {
        throw IllegalArgumentException("Owner field $name is not an unsigned integer")
    }
    return value
}

private fun parseNullableUnsignedDecimal(rawValue: String, name: String): String? {
    val value = rawValue.trim()
    if (value == "null") return null
    if (!value.matches(Regex("0|[1-9][0-9]*")) || value.length > 39) {
        throw IllegalArgumentException("Owner field $name is not an unsigned integer")
    }
    val parsed = value.toBigIntegerOrNull()
    if (parsed == null || parsed > MAX_U128) {
        throw IllegalArgumentException("Owner field $name is outside the unsigned 128-bit range")
    }
    return value
}

internal fun JSONObject.singleVariant(): String = keys().asSequence().singleOrNull()
    ?: throw IllegalArgumentException("Tagged Rust enum must contain exactly one variant")

internal fun JSONObject.optionalObject(name: String): JSONObject? {
    val value = opt(name) ?: return null
    if (value === JSONObject.NULL) return null
    return value as? JSONObject ?: throw IllegalArgumentException("Owner field $name is not an object")
}

private val MAX_U128 = BigInteger("340282366920938463463374607431768211455")

/** Read one raw JSON value without converting large unsigned integers through Double. */
internal fun rawJsonValueAtPath(json: String, vararg path: String): String? {
    var start = skipJsonWhitespace(json, 0)
    var end = json.length
    for (key in path) {
        val range = when (json[start]) {
            '{' -> rawObjectPropertyRange(json, start, end, key)
            '[' -> rawArrayElementRange(json, start, end, key)
            else -> null
        } ?: return null
        start = range.first
        end = range.second
    }
    return json.substring(start, end)
}

private fun rawArrayElementRange(json: String, arrayStart: Int, arrayEnd: Int, requestedIndex: String): Pair<Int, Int>? {
    val index = requestedIndex.toIntOrNull()?.takeIf { it >= 0 } ?: return null
    var cursor = skipJsonWhitespace(json, arrayStart)
    require(cursor < arrayEnd && json[cursor] == '[') { "Expected a JSON array" }
    cursor++
    var currentIndex = 0
    while (true) {
        cursor = skipJsonWhitespace(json, cursor)
        if (cursor >= arrayEnd || json[cursor] == ']') return null
        val valueStart = cursor
        val valueEnd = skipJsonValue(json, valueStart, arrayEnd)
        if (currentIndex == index) return valueStart to valueEnd
        currentIndex++
        cursor = skipJsonWhitespace(json, valueEnd)
        if (cursor < arrayEnd && json[cursor] == ',') {
            cursor++
        } else {
            return null
        }
    }
}

private fun rawObjectPropertyRange(json: String, objectStart: Int, objectEnd: Int, requestedKey: String): Pair<Int, Int>? {
    var cursor = skipJsonWhitespace(json, objectStart)
    require(cursor < objectEnd && json[cursor] == '{') { "Expected a JSON object" }
    cursor++
    while (true) {
        cursor = skipJsonWhitespace(json, cursor)
        if (cursor >= objectEnd || json[cursor] == '}') return null
        require(json[cursor] == '"') { "Expected a JSON object key" }
        val keyEnd = skipJsonString(json, cursor)
        val key = json.substring(cursor + 1, keyEnd - 1)
        cursor = skipJsonWhitespace(json, keyEnd)
        require(cursor < objectEnd && json[cursor] == ':') { "Expected a JSON object separator" }
        val valueStart = skipJsonWhitespace(json, cursor + 1)
        val valueEnd = skipJsonValue(json, valueStart, objectEnd)
        if (key == requestedKey) return valueStart to valueEnd
        cursor = skipJsonWhitespace(json, valueEnd)
        if (cursor < objectEnd && json[cursor] == ',') {
            cursor++
        } else {
            return null
        }
    }
}

private fun skipJsonValue(json: String, start: Int, bound: Int): Int {
    require(start < bound) { "Expected a JSON value" }
    if (json[start] == '"') return skipJsonString(json, start)
    if (json[start] != '{' && json[start] != '[') {
        var cursor = start
        while (cursor < bound && json[cursor] != ',' && json[cursor] != '}' && json[cursor] != ']' &&
            !json[cursor].isWhitespace()
        ) {
            cursor++
        }
        require(cursor > start) { "Expected a JSON value" }
        return cursor
    }

    var depth = 0
    var cursor = start
    while (cursor < bound) {
        when (json[cursor]) {
            '"' -> cursor = skipJsonString(json, cursor)

            '{', '[' -> {
                depth++
                cursor++
            }

            '}', ']' -> {
                depth--
                cursor++
                if (depth == 0) return cursor
                require(depth > 0) { "Malformed JSON container" }
            }

            else -> cursor++
        }
    }
    throw IllegalArgumentException("Unterminated JSON container")
}

private fun skipJsonString(json: String, start: Int): Int {
    require(start < json.length && json[start] == '"') { "Expected a JSON string" }
    var cursor = start + 1
    while (cursor < json.length) {
        when (json[cursor]) {
            '\\' -> cursor += 2
            '"' -> return cursor + 1
            else -> cursor++
        }
    }
    throw IllegalArgumentException("Unterminated JSON string")
}

private fun skipJsonWhitespace(json: String, start: Int): Int {
    var cursor = start
    while (cursor < json.length && json[cursor].isWhitespace()) cursor++
    return cursor
}

internal fun JSONObject.nullableString(name: String): String? {
    val value = opt(name) ?: return null
    if (value === JSONObject.NULL) return null
    return value as? String ?: throw IllegalArgumentException("Owner field $name is not a string")
}

internal fun JSONObject.unsigned(name: String): ULong = opt(name)
    ?.toString()
    ?.removeSuffix(".0")
    ?.toULongOrNull()
    ?: throw IllegalArgumentException("Owner field $name is not an unsigned integer")

internal fun JSONObject.rate(name: String): ULong {
    val value = (opt(name) as? Number)?.toDouble()
        ?: throw IllegalArgumentException("Owner field $name is not numeric")
    if (!value.isFinite()) {
        throw IllegalArgumentException("Owner field $name is not finite")
    }
    return value.roundToLong().coerceAtLeast(0).toULong()
}

internal fun JSONArray.strings(): List<String> = (0 until length()).map { index ->
    getString(index)
}

internal inline fun <T> JSONArray.mapObjects(transform: (JSONObject) -> T): List<T> = (0 until length()).map { index ->
    transform(getJSONObject(index))
}

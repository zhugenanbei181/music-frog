package com.musicfrog.infiltrator.ui.settings.rules

import infiltrator_android.AndroidJsonEntry
import infiltrator_android.AndroidJsonValue
import infiltrator_android.AndroidRuleProviderEntry
import org.json.JSONArray
import org.json.JSONObject

private const val MAX_PROVIDER_JSON_DEPTH = 128
private val JSON_NUMBER = Regex("-?(?:0|[1-9][0-9]*)(?:\\.[0-9]+)?(?:[eE][+-]?[0-9]+)?")

internal fun ruleProvidersJsonToEntries(json: String): List<AndroidRuleProviderEntry> =
    ProviderDocumentParser(json).parse()

internal fun ruleProviderEntriesToJson(entries: List<AndroidRuleProviderEntry>): String {
    val seenNames = HashSet<String>()
    val providers = JSONObject()
    entries.sortedBy(AndroidRuleProviderEntry::name).forEach { entry ->
        require(seenNames.add(entry.name)) { "Duplicate rule provider name: ${entry.name}" }
        providers.put(entry.name, ownerValueToJsonValue(entry.configuration, 0))
    }
    return providers.toString(2)
}

private fun ownerValueToJsonValue(value: AndroidJsonValue, depth: Int): Any {
    require(depth <= MAX_PROVIDER_JSON_DEPTH) {
        "Rule provider JSON nesting exceeds the supported depth"
    }
    return when (value) {
        AndroidJsonValue.Null -> JSONObject.NULL
        is AndroidJsonValue.BoolValue -> value.value
        is AndroidJsonValue.NumberValue -> value.value.toBigDecimalOrNull()
            ?: throw IllegalArgumentException("Invalid provider number: ${value.value}")
        is AndroidJsonValue.StringValue -> value.value
        is AndroidJsonValue.ArrayValue -> JSONArray().also { array ->
            value.value.forEach { array.put(ownerValueToJsonValue(it, depth + 1)) }
        }
        is AndroidJsonValue.ObjectValue -> JSONObject().also { objectValue ->
            val seenKeys = HashSet<String>()
            value.value.forEach { entry ->
                require(seenKeys.add(entry.key)) { "Duplicate provider field: ${entry.key}" }
                objectValue.put(entry.key, ownerValueToJsonValue(entry.value, depth + 1))
            }
        }
    }
}

private class ProviderDocumentParser(private val source: String) {
    private var offset = 0

    fun parse(): List<AndroidRuleProviderEntry> {
        skipWhitespace()
        val entries = parseObjectEntries(valueDepth = 0)
        skipWhitespace()
        require(offset == source.length) { "Unexpected content after rule providers document" }
        return entries.map { entry -> AndroidRuleProviderEntry(entry.key, entry.value) }
    }

    private fun parseValue(depth: Int): AndroidJsonValue {
        require(depth <= MAX_PROVIDER_JSON_DEPTH) {
            "Rule provider JSON nesting exceeds the supported depth"
        }
        skipWhitespace()
        require(offset < source.length) { "Unexpected end of rule providers document" }
        return when (source[offset]) {
            'n' -> {
                consumeLiteral("null")
                AndroidJsonValue.Null
            }
            't' -> {
                consumeLiteral("true")
                AndroidJsonValue.BoolValue(true)
            }
            'f' -> {
                consumeLiteral("false")
                AndroidJsonValue.BoolValue(false)
            }
            '"' -> AndroidJsonValue.StringValue(readString())
            '{' -> AndroidJsonValue.ObjectValue(parseObjectEntries(valueDepth = depth + 1))
            '[' -> AndroidJsonValue.ArrayValue(parseArrayValues(valueDepth = depth + 1))
            '-', in '0'..'9' -> parseNumber()
            else -> fail("Invalid rule provider JSON value")
        }
    }

    private fun parseObjectEntries(valueDepth: Int): List<AndroidJsonEntry> {
        expect('{')
        skipWhitespace()
        if (consumeIf('}')) return emptyList()

        val seenKeys = HashSet<String>()
        val entries = mutableListOf<AndroidJsonEntry>()
        while (true) {
            skipWhitespace()
            require(peek() == '"') { "Expected a quoted key in rule providers document" }
            val key = readString()
            require(seenKeys.add(key)) { "Duplicate provider field: $key" }
            skipWhitespace()
            expect(':')
            entries += AndroidJsonEntry(key, parseValue(valueDepth))
            skipWhitespace()
            if (consumeIf('}')) return entries.sortedBy(AndroidJsonEntry::key)
            expect(',')
        }
    }

    private fun parseArrayValues(valueDepth: Int): List<AndroidJsonValue> {
        expect('[')
        skipWhitespace()
        if (consumeIf(']')) return emptyList()

        val values = mutableListOf<AndroidJsonValue>()
        while (true) {
            values += parseValue(valueDepth)
            skipWhitespace()
            if (consumeIf(']')) return values
            expect(',')
        }
    }

    private fun parseNumber(): AndroidJsonValue.NumberValue {
        val start = offset
        while (offset < source.length && source[offset] in "-+0123456789.eE") offset += 1
        val number = source.substring(start, offset)
        require(JSON_NUMBER.matches(number) && number.toBigDecimalOrNull() != null) {
            "Invalid rule provider number: $number"
        }
        return AndroidJsonValue.NumberValue(number)
    }

    private fun readString(): String {
        expect('"')
        val value = StringBuilder()
        while (offset < source.length) {
            val char = source[offset++]
            when {
                char == '"' -> return value.toString()
                char == '\\' -> readEscape(value)
                char.code < 0x20 -> fail("Unescaped control character in rule providers document")
                else -> value.append(char)
            }
        }
        fail("Unterminated string in rule providers document")
    }

    private fun readEscape(value: StringBuilder) {
        require(offset < source.length) { "Incomplete escape in rule providers document" }
        when (val escape = source[offset++]) {
            '"', '\\', '/' -> value.append(escape)
            'b' -> value.append('\b')
            'f' -> value.append('\u000c')
            'n' -> value.append('\n')
            'r' -> value.append('\r')
            't' -> value.append('\t')
            'u' -> {
                require(offset + 4 <= source.length) { "Incomplete Unicode escape in rule providers document" }
                val codeUnit = source.substring(offset, offset + 4).toIntOrNull(16)
                    ?: fail("Invalid Unicode escape in rule providers document")
                value.append(codeUnit.toChar())
                offset += 4
            }
            else -> fail("Invalid escape in rule providers document")
        }
    }

    private fun consumeLiteral(literal: String) {
        require(source.startsWith(literal, offset)) { "Invalid rule provider JSON literal" }
        offset += literal.length
    }

    private fun skipWhitespace() {
        while (offset < source.length && source[offset] in " \t\n\r") offset += 1
    }

    private fun peek(): Char? = source.getOrNull(offset)

    private fun expect(char: Char) {
        require(offset < source.length && source[offset] == char) {
            "Expected '$char' in rule providers document"
        }
        offset += 1
    }

    private fun consumeIf(char: Char): Boolean {
        if (peek() != char) return false
        offset += 1
        return true
    }

    private fun fail(message: String): Nothing = throw IllegalArgumentException(message)
}

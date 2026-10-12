package com.musicfrog.infiltrator

import java.io.File
import java.security.MessageDigest
import org.json.JSONObject

internal sealed interface CoreBinaryResolution {
    data class Available(val file: File) : CoreBinaryResolution

    data object Missing : CoreBinaryResolution

    data class Rejected(val reason: String) : CoreBinaryResolution
}

internal class AndroidCoreBinaryResolver(
    private val nativeLibraryDirectory: File,
    private val filesDirectory: File,
) {
    fun resolve(binary: CoreLaunchBinaryWire): CoreBinaryResolution = when (binary) {
        CoreLaunchBinaryWire.MihomoBundled -> resolveBundledMihomo()
        is CoreLaunchBinaryWire.SingBoxManaged -> resolveManagedSingBox(binary)
    }

    private fun resolveBundledMihomo(): CoreBinaryResolution {
        val executable = File(nativeLibraryDirectory, MIHOMO_BINARY_NAME)
        return when {
            !executable.exists() -> CoreBinaryResolution.Missing
            executable.isFile -> CoreBinaryResolution.Available(executable)
            else -> CoreBinaryResolution.Rejected("bundled Mihomo path is not a file")
        }
    }

    private fun resolveManagedSingBox(
        binary: CoreLaunchBinaryWire.SingBoxManaged,
    ): CoreBinaryResolution {
        if (!isCanonicalSingBoxVersion(binary.version)) {
            return CoreBinaryResolution.Rejected("managed sing-box version is not canonical")
        }
        val root = filesDirectory.canonicalFile
        val versionDirectory = File(
            root,
            "engines/sing-box/targets/${binary.target.targetTriple}/versions/${binary.version}",
        ).canonicalFile
        if (!versionDirectory.isDescendantOf(root)) {
            return CoreBinaryResolution.Rejected("managed sing-box version path escapes app storage")
        }
        val executable = File(versionDirectory, SING_BOX_BINARY_NAME)
        val receipt = File(versionDirectory, INSTALL_RECEIPT_NAME)
        if (!executable.exists()) return CoreBinaryResolution.Missing
        if (!executable.isFile || executable.canonicalFile.parentFile != versionDirectory) {
            return CoreBinaryResolution.Rejected("managed sing-box executable path is invalid")
        }
        if (!executable.canExecute()) {
            return CoreBinaryResolution.Rejected("managed sing-box executable is not executable")
        }
        if (!receipt.isFile || receipt.length() > MAX_INSTALL_RECEIPT_BYTES ||
            receipt.canonicalFile.parentFile != versionDirectory
        ) {
            return CoreBinaryResolution.Rejected("managed sing-box installation receipt is missing or invalid")
        }

        return runCatching {
            val stored = JSONObject(receipt.readText(Charsets.UTF_8))
            require(stored.keys().asSequence().toSet() == INSTALL_RECEIPT_FIELDS)
            require(stored.getInt("schema_version") == INSTALL_SCHEMA_VERSION)
            require(stored.requiredString("engine_id") == SING_BOX_ENGINE_ID)
            require(stored.requiredString("version") == binary.version)
            require(stored.requiredString("target") == binary.target.targetTriple)
            require(stored.getInt("adapter_revision") > 0)
            require(stored.requiredString("archive").isNotBlank())
            require(stored.requiredString("archive_sha256").isSha256Digest())
            val expectedDigest = stored.requiredString("binary_sha256")
            require(expectedDigest.isSha256Digest())
            require(executable.sha256() == expectedDigest)
            CoreBinaryResolution.Available(executable)
        }.getOrElse { error ->
            CoreBinaryResolution.Rejected(
                error.message ?: "managed sing-box installation receipt is invalid",
            )
        }
    }

    private fun isCanonicalSingBoxVersion(value: String): Boolean {
        if (!value.startsWith('v')) return false
        val canonical = value.drop(1)
        val numeric = canonical.takeWhile { it != '-' && it != '+' }
        val components = numeric.split('.')
        return canonical.isNotEmpty() &&
            components.size == 3 &&
            components.all { component ->
                component.isNotEmpty() && component.all { it in '0'..'9' }
            } &&
            canonical.all { character ->
                character in 'a'..'z' ||
                    character in 'A'..'Z' ||
                    character in '0'..'9' ||
                    character == '.' ||
                    character == '-' ||
                    character == '+'
            }
    }

    private fun File.isDescendantOf(root: File): Boolean =
        path.startsWith(root.path.trimEnd(File.separatorChar) + File.separator)

    private fun String.isSha256Digest(): Boolean =
        length == SHA256_HEX_LENGTH && all { it in '0'..'9' || it in 'a'..'f' }

    private fun File.sha256(): String {
        val digest = MessageDigest.getInstance("SHA-256")
        inputStream().buffered().use { stream ->
            val buffer = ByteArray(DIGEST_BUFFER_BYTES)
            while (true) {
                val count = stream.read(buffer)
                if (count < 0) break
                digest.update(buffer, 0, count)
            }
        }
        return digest.digest().joinToString("") { byte -> "%02x".format(byte.toInt() and 0xff) }
    }

    private fun JSONObject.requiredString(key: String): String =
        get(key) as? String ?: error("${key} must be a string")

    private companion object {
        const val MIHOMO_BINARY_NAME = "libmihomo.so"
        const val SING_BOX_BINARY_NAME = "sing-box"
        const val INSTALL_RECEIPT_NAME = "install.json"
        const val SING_BOX_ENGINE_ID = "sing-box"
        const val INSTALL_SCHEMA_VERSION = 1
        const val SHA256_HEX_LENGTH = 64
        const val DIGEST_BUFFER_BYTES = 64 * 1024
        const val MAX_INSTALL_RECEIPT_BYTES = 16 * 1024L
        val INSTALL_RECEIPT_FIELDS = setOf(
            "schema_version",
            "engine_id",
            "version",
            "target",
            "adapter_revision",
            "archive",
            "archive_sha256",
            "binary_sha256",
        )
    }
}

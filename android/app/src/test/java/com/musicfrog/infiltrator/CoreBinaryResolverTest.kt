package com.musicfrog.infiltrator

import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File
import java.nio.file.Files
import java.security.MessageDigest

class CoreBinaryResolverTest {
    @Test
    fun resolvesBundledMihomoFromNativeLibraryDirectory() = withDirectory { root ->
        val nativeDirectory = File(root, "native").apply { mkdirs() }
        val binary = File(nativeDirectory, "libmihomo.so").apply {
            writeBytes(byteArrayOf(1, 2, 3))
        }
        val resolver = AndroidCoreBinaryResolver(nativeDirectory, File(root, "files"))

        assertEquals(
            CoreBinaryResolution.Available(binary),
            resolver.resolve(CoreLaunchBinaryWire.MihomoBundled),
        )
    }

    @Test
    fun resolvesSingBoxOnlyWhenTargetVersionReceiptAndDigestMatch() = withDirectory { root ->
        val files = File(root, "files").apply { mkdirs() }
        val binary = managedBinary(files, "aarch64-linux-android", "v1.14.3")
        val resolver = AndroidCoreBinaryResolver(File(root, "native"), files)
        val descriptor = CoreLaunchBinaryWire.SingBoxManaged(
            "v1.14.3",
            AndroidCoreBinaryTargetWire.Arm64V8a,
        )

        assertEquals(CoreBinaryResolution.Available(binary), resolver.resolve(descriptor))

        val receipt = File(binary.parentFile, "install.json")
        val changed = JSONObject(receipt.readText()).put("target", "x86_64-linux-android")
        receipt.writeText(changed.toString())
        assertTrue(resolver.resolve(descriptor) is CoreBinaryResolution.Rejected)
    }

    @Test
    fun rejectsUntrustedVersionPathsAndChangedExecutableBytes() = withDirectory { root ->
        val files = File(root, "files").apply { mkdirs() }
        val resolver = AndroidCoreBinaryResolver(File(root, "native"), files)
        val traversal = CoreLaunchBinaryWire.SingBoxManaged(
            "v1.14.3/../../escape",
            AndroidCoreBinaryTargetWire.Arm64V8a,
        )
        assertTrue(resolver.resolve(traversal) is CoreBinaryResolution.Rejected)
        assertEquals(
            CoreBinaryResolution.Missing,
            resolver.resolve(
                CoreLaunchBinaryWire.SingBoxManaged(
                    "v1.14.3",
                    AndroidCoreBinaryTargetWire.Arm64V8a,
                ),
            ),
        )

        val binary = managedBinary(files, "aarch64-linux-android", "v1.14.3")
        binary.appendBytes(byteArrayOf(4))
        val changedBytes = CoreLaunchBinaryWire.SingBoxManaged(
            "v1.14.3",
            AndroidCoreBinaryTargetWire.Arm64V8a,
        )
        assertTrue(resolver.resolve(changedBytes) is CoreBinaryResolution.Rejected)
    }

    private fun managedBinary(files: File, targetTriple: String, version: String): File {
        val versionDirectory = File(
            files,
            "engines/sing-box/targets/$targetTriple/versions/$version",
        ).apply { mkdirs() }
        val binary = File(versionDirectory, "sing-box").apply {
            writeBytes(byteArrayOf(7, 8, 9))
            setExecutable(true)
        }
        File(versionDirectory, "install.json").writeText(
            JSONObject()
                .put("schema_version", 1)
                .put("engine_id", "sing-box")
                .put("version", version)
                .put("target", targetTriple)
                .put("adapter_revision", 1)
                .put("archive", "sing-box.tar.gz")
                .put("archive_sha256", digest(byteArrayOf(3, 4, 5)))
                .put("binary_sha256", digest(binary.readBytes()))
                .toString(),
        )
        return binary
    }

    private fun digest(bytes: ByteArray): String = MessageDigest.getInstance("SHA-256")
        .digest(bytes)
        .joinToString("") { byte -> "%02x".format(byte.toInt() and 0xff) }

    private fun withDirectory(block: (File) -> Unit) {
        val root = Files.createTempDirectory("android-core-binary-").toFile()
        try {
            block(root)
        } finally {
            root.deleteRecursively()
        }
    }
}

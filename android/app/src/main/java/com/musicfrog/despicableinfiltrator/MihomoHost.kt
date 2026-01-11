package com.musicfrog.despicableinfiltrator

import android.content.Context
import android.content.SharedPreferences
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Base64
import android.util.Log
import java.io.File
import java.io.IOException
import java.nio.ByteBuffer
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

private const val TAG = "MihomoHost"
private const val ASSET_NAME = "mihomo/mihomo-android-arm64-v8"
private const val BIN_DIR = "mihomo"
private const val BIN_NAME = "mihomo-android-arm64-v8"
private const val CONFIG_DIR = "configs"
private const val CONFIG_NAME = "default.yaml"
private const val CONTROLLER_ADDR = "127.0.0.1:9090"
private const val CONTROLLER_URL = "http://127.0.0.1:9090"
private const val CREDENTIALS_PREFS = "credentials"
private const val CREDENTIALS_KEY_ALIAS = "mfdi_credentials_key"
private const val CREDENTIALS_FORMAT_PREFIX = "v1:"
private const val CREDENTIALS_GCM_TAG_BITS = 128

class MihomoHost(private val context: Context) : BridgeHost {
    private val processManager = MihomoProcessManager(context)
    private val credentialStore = AndroidCredentialStore(context)

    override fun coreStart(): Boolean {
        val configFile = processManager.ensureConfigFile()
        return processManager.start(configFile)
    }

    override fun coreStop(): Boolean {
        return processManager.stop()
    }

    override fun coreIsRunning(): Boolean {
        return processManager.isRunning()
    }

    override fun coreControllerUrl(): String? {
        return CONTROLLER_URL
    }

    override fun credentialGet(service: String, key: String): String? {
        return credentialStore.get(service, key)
    }

    override fun credentialSet(service: String, key: String, value: String): Boolean {
        return credentialStore.set(service, key, value)
    }

    override fun credentialDelete(service: String, key: String): Boolean {
        return credentialStore.delete(service, key)
    }

    override fun dataDir(): String? {
        return context.filesDir.absolutePath
    }

    override fun cacheDir(): String? {
        return context.cacheDir.absolutePath
    }

    override fun vpnStart(): Boolean {
        if (!coreStart()) {
            return false
        }
        return MihomoVpnService.start(context)
    }

    override fun vpnStop(): Boolean {
        val stopped = MihomoVpnService.stop(context)
        val coreStopped = coreStop()
        return stopped && coreStopped
    }

    override fun vpnIsRunning(): Boolean {
        return MihomoVpnService.isRunning()
    }

    override fun tunSetEnabled(enabled: Boolean): Boolean {
        return if (enabled) vpnStart() else vpnStop()
    }

    override fun tunIsEnabled(): Boolean {
        return vpnIsRunning()
    }
}

private class MihomoProcessManager(private val context: Context) {
    @Volatile
    private var process: Process? = null

    @Synchronized
    fun start(configFile: File): Boolean {
        if (process?.isAlive == true) {
            return true
        }
        val binary = ensureBinary() ?: return false
        return try {
            val builder = ProcessBuilder(
                binary.absolutePath,
                "-d",
                context.filesDir.absolutePath,
                "-f",
                configFile.absolutePath
            )
                .directory(context.filesDir)
                .redirectErrorStream(true)
            
            val p = builder.start()
            process = p
            
            // Start a thread to consume stdout/stderr to prevent blocking and log output
            Thread {
                try {
                    p.inputStream.bufferedReader().use { reader ->
                        reader.forEachLine { line ->
                            Log.d("MihomoCore", line)
                        }
                    }
                } catch (e: Exception) {
                    Log.w(TAG, "log reader failed: ${e.message}")
                }
            }.start()

            // Wait a bit to see if it crashes immediately
            Thread.sleep(500)
            if (p.isAlive) {
                true
            } else {
                val exitCode = try { p.exitValue() } catch(e: Exception) { -1 }
                Log.w(TAG, "core exited immediately with code $exitCode")
                process = null
                false
            }
        } catch (err: IOException) {
            Log.w(TAG, "start core failed: ${err.message}")
            false
        }
    }

    @Synchronized
    fun stop(): Boolean {
        val current = process ?: return true
        if (!current.isAlive) {
            process = null
            return true
        }
        current.destroy()
        return try {
            current.waitFor()
            process = null
            true
        } catch (err: InterruptedException) {
            Log.w(TAG, "stop core interrupted: ${err.message}")
            false
        }
    }

    fun isRunning(): Boolean {
        return process?.isAlive == true
    }

    fun ensureConfigFile(): File {
        val configDir = File(context.filesDir, CONFIG_DIR)
        if (!configDir.exists()) {
            configDir.mkdirs()
        }
        val configFile = File(configDir, CONFIG_NAME)
        if (!configFile.exists()) {
            configFile.writeText(defaultConfig())
        }
        return configFile
    }

    private fun ensureBinary(): File? {
        // Android 10+ requires executing binaries from nativeLibraryDir
        val nativeLibraryDir = context.applicationInfo.nativeLibraryDir
        val binary = File(nativeLibraryDir, "libmihomo.so")
        if (binary.exists()) {
            return binary
        }
        Log.w(TAG, "native binary not found at ${binary.absolutePath}")
        return null
    }

    private fun defaultConfig(): String {
        return """
            port: 7890
            socks-port: 7891
            allow-lan: false
            mode: rule
            log-level: info
            external-controller: $CONTROLLER_ADDR
            
            tun:
              enable: false
        """.trimIndent() + "\n"
    }
}

private class AndroidCredentialStore(context: Context) {
    private val prefs: SharedPreferences = context.getSharedPreferences(
        CREDENTIALS_PREFS,
        Context.MODE_PRIVATE
    )

    fun get(service: String, key: String): String? {
        val payload = prefs.getString(combineKey(service, key), null) ?: return null
        return decrypt(payload)
    }

    fun set(service: String, key: String, value: String): Boolean {
        val payload = encrypt(value) ?: return false
        return prefs.edit().putString(combineKey(service, key), payload).commit()
    }

    fun delete(service: String, key: String): Boolean {
        return prefs.edit().remove(combineKey(service, key)).commit()
    }

    private fun combineKey(service: String, key: String): String {
        return "$service:$key"
    }

    private fun encrypt(value: String): String? {
        val key = getOrCreateSecretKey() ?: return null
        return try {
            val cipher = Cipher.getInstance("AES/GCM/NoPadding")
            cipher.init(Cipher.ENCRYPT_MODE, key)
            val iv = cipher.iv
            val cipherText = cipher.doFinal(value.toByteArray(Charsets.UTF_8))
            val buffer = ByteBuffer.allocate(1 + iv.size + cipherText.size)
            buffer.put(iv.size.toByte())
            buffer.put(iv)
            buffer.put(cipherText)
            CREDENTIALS_FORMAT_PREFIX + Base64.encodeToString(buffer.array(), Base64.NO_WRAP)
        } catch (err: Exception) {
            Log.w(TAG, "encrypt credential failed: ${err.message}")
            null
        }
    }

    private fun decrypt(payload: String): String? {
        if (!payload.startsWith(CREDENTIALS_FORMAT_PREFIX)) {
            return null
        }
        val encoded = payload.removePrefix(CREDENTIALS_FORMAT_PREFIX)
        return try {
            val raw = Base64.decode(encoded, Base64.NO_WRAP)
            if (raw.isEmpty()) {
                return null
            }
            val ivLength = raw[0].toInt() and 0xff
            val ivStart = 1
            val ivEnd = ivStart + ivLength
            if (ivLength <= 0 || ivEnd >= raw.size) {
                return null
            }
            val iv = raw.copyOfRange(ivStart, ivEnd)
            val cipherText = raw.copyOfRange(ivEnd, raw.size)
            val key = getOrCreateSecretKey() ?: return null
            val cipher = Cipher.getInstance("AES/GCM/NoPadding")
            val spec = GCMParameterSpec(CREDENTIALS_GCM_TAG_BITS, iv)
            cipher.init(Cipher.DECRYPT_MODE, key, spec)
            String(cipher.doFinal(cipherText), Charsets.UTF_8)
        } catch (err: Exception) {
            Log.w(TAG, "decrypt credential failed: ${err.message}")
            null
        }
    }

    private fun getOrCreateSecretKey(): SecretKey? {
        return try {
            val keyStore = KeyStore.getInstance("AndroidKeyStore")
            keyStore.load(null)
            val existing = keyStore.getKey(CREDENTIALS_KEY_ALIAS, null) as? SecretKey
            if (existing != null) {
                return existing
            }
            val keyGen = KeyGenerator.getInstance(
                KeyProperties.KEY_ALGORITHM_AES,
                "AndroidKeyStore"
            )
            val spec = KeyGenParameterSpec.Builder(
                CREDENTIALS_KEY_ALIAS,
                KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT
            )
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                .setKeySize(256)
                .build()
            keyGen.init(spec)
            keyGen.generateKey()
        } catch (err: Exception) {
            Log.w(TAG, "keystore unavailable: ${err.message}")
            null
        }
    }
}

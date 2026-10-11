package com.musicfrog.infiltrator

import android.content.Context
import android.content.SharedPreferences
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Base64
import android.util.Log
import java.nio.ByteBuffer
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

internal class AndroidCredentialStore(context: Context) {
    private val prefs: SharedPreferences = context.getSharedPreferences(
        CREDENTIALS_PREFS,
        Context.MODE_PRIVATE,
    )

    fun get(service: String, key: String): String? {
        val payload = prefs.getString(combineKey(service, key), null) ?: return null
        return decrypt(payload)
    }

    fun set(service: String, key: String, value: String): Boolean {
        val payload = encrypt(value) ?: return false
        return prefs.edit().putString(combineKey(service, key), payload).commit()
    }

    fun delete(service: String, key: String): Boolean =
        prefs.edit().remove(combineKey(service, key)).commit()

    private fun combineKey(service: String, key: String): String = "$service:$key"

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
        } catch (error: Exception) {
            Log.w(TAG, "Credential encryption failed: ${error.message}")
            null
        }
    }

    private fun decrypt(payload: String): String? {
        if (!payload.startsWith(CREDENTIALS_FORMAT_PREFIX)) return null
        val encoded = payload.removePrefix(CREDENTIALS_FORMAT_PREFIX)
        return try {
            val raw = Base64.decode(encoded, Base64.NO_WRAP)
            if (raw.isEmpty()) return null
            val ivLength = raw[0].toInt() and 0xff
            val ivStart = 1
            val ivEnd = ivStart + ivLength
            if (ivLength <= 0 || ivEnd >= raw.size) return null
            val iv = raw.copyOfRange(ivStart, ivEnd)
            val cipherText = raw.copyOfRange(ivEnd, raw.size)
            val key = getOrCreateSecretKey() ?: return null
            val cipher = Cipher.getInstance("AES/GCM/NoPadding")
            cipher.init(Cipher.DECRYPT_MODE, key, GCMParameterSpec(CREDENTIALS_GCM_TAG_BITS, iv))
            String(cipher.doFinal(cipherText), Charsets.UTF_8)
        } catch (error: Exception) {
            Log.w(TAG, "Credential decryption failed: ${error.message}")
            null
        }
    }

    private fun getOrCreateSecretKey(): SecretKey? = try {
        val keyStore = KeyStore.getInstance("AndroidKeyStore")
        keyStore.load(null)
        val existing = keyStore.getKey(CREDENTIALS_KEY_ALIAS, null) as? SecretKey
        if (existing != null) {
            existing
        } else {
            val keyGenerator = KeyGenerator.getInstance(
                KeyProperties.KEY_ALGORITHM_AES,
                "AndroidKeyStore",
            )
            val spec = KeyGenParameterSpec.Builder(
                CREDENTIALS_KEY_ALIAS,
                KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT,
            )
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                .setKeySize(256)
                .build()
            keyGenerator.init(spec)
            keyGenerator.generateKey()
        }
    } catch (error: Exception) {
        Log.w(TAG, "Android Keystore is unavailable: ${error.message}")
        null
    }

    private companion object {
        const val TAG = "AndroidCredentialStore"
        const val CREDENTIALS_PREFS = "credentials"
        const val CREDENTIALS_KEY_ALIAS = "mfdi_credentials_key"
        const val CREDENTIALS_FORMAT_PREFIX = "v1:"
        const val CREDENTIALS_GCM_TAG_BITS = 128
    }
}

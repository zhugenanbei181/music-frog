package com.musicfrog.infiltrator

import org.junit.Assert.assertTrue
import java.io.File

/**
 * 读取 Rust owner 源码里的 `as_code` 线码。
 *
 * Kotlin 与 Rust 之间的 wire 线码必须逐个对齐，这个读取器让对齐测试可以
 * 直接对着 owner 的源码验证，而不是复制一份数字。
 */
internal object RustOwnerWireCodes {
    /** 读取仓库里一份 Rust owner 源码。 */
    fun read(relativePath: String): String = File(relativePath).readText()

    /**
     * 读取 `as_code` 的线码，顺序就是枚举的声明顺序。
     */
    fun asCodeCodes(source: String, implHeader: String): List<Int> {
        val body = source.substringAfter(implHeader).substringBefore("\n}")
        val codes = Regex("""=>\s*(\d+)""")
            .findAll(body)
            .map { it.groupValues[1].toInt() }
            .toList()
        assertTrue("the Rust owner must publish wire codes", codes.isNotEmpty())
        return codes
    }
}

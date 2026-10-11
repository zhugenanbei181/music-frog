package com.musicfrog.infiltrator

import org.junit.Assert.assertEquals
import org.junit.Test

class AndroidProcessRoleTest {
    @Test
    fun packageProcessIsTheUiAdapter() {
        assertEquals(AndroidProcessRole.Ui, AndroidProcessRole.identify(PACKAGE, PACKAGE))
    }

    @Test
    fun vpnProcessIsTheOnlyCoreOwner() {
        assertEquals(
            AndroidProcessRole.VpnOwner,
            AndroidProcessRole.identify(PACKAGE, "$PACKAGE:vpn"),
        )
    }

    @Test
    fun unrelatedProcessCannotAccidentallyOwnTheCore() {
        val role = AndroidProcessRole.identify(PACKAGE, "$PACKAGE:worker")
        assertEquals(AndroidProcessRole.Unsupported("$PACKAGE:worker"), role)
        assertEquals(2, role.toWireCode())
    }

    @Test
    fun processRoleWireCodesDistinguishUiOwnerAndUnsupportedProcesses() {
        assertEquals(0, AndroidProcessRole.Ui.toWireCode())
        assertEquals(1, AndroidProcessRole.VpnOwner.toWireCode())
        assertEquals(2, AndroidProcessRole.Unsupported("worker").toWireCode())
    }

    private companion object {
        const val PACKAGE = "com.musicfrog.infiltrator"
    }
}

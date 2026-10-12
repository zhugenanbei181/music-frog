package com.musicfrog.infiltrator

import org.junit.Assert.assertEquals
import org.junit.Test

class VpnPreparedConfigurationTest {
    @Test
    fun parserRequiresTheTypedScopeAndFrozenBuilderConfiguration() {
        val prepared = NativePreparedVpnConfiguration.parse(PREPARED_CONFIGURATION)

        assertEquals(21uL, prepared?.preparationId)
        assertEquals(8uL, prepared?.vpnGeneration)
        assertEquals(VpnOwnerDataRevision.fromWire(44uL), prepared?.ownerDataRevision)
        assertEquals("mihomo", prepared?.coreScope?.engineId)
        assertEquals("primary", prepared?.coreScope?.instanceId)
        assertEquals(17uL, prepared?.coreScope?.generation)
        assertEquals("340282366920938463463374607431768211455", prepared?.coreScope?.sessionToken)
        assertEquals(1420, prepared?.configuration?.mtu)
        assertEquals(listOf("1.1.1.1"), prepared?.configuration?.dnsServers)

        val legacyBuilderOnlyJson = """{"proxy_endpoint":"socks5://127.0.0.1:7891"}"""
        assertEquals(null, NativePreparedVpnConfiguration.parse(legacyBuilderOnlyJson))
        assertEquals(
            null,
            NativePreparedVpnConfiguration.parse(
                PREPARED_CONFIGURATION.replace("\"owner_data_revision\": 44,", ""),
            ),
        )
        assertEquals(
            null,
            NativePreparedVpnConfiguration.parse(
                PREPARED_CONFIGURATION.replace("\"owner_data_revision\": 44", "\"owner_data_revision\": 0"),
            ),
        )
        assertEquals(
            null,
            NativePreparedVpnConfiguration.parse(
                PREPARED_CONFIGURATION.replace(
                    "\"vpn_generation\": 8,",
                    "\"vpn_generation\": 8, \"extra\": true,",
                ),
            ),
        )
    }

    @Test
    fun routeAndDnsParsingAcceptsOnlyNumericIpv4OrIpv6Literals() {
        val ipv6 = requireNotNull(
            NativePreparedVpnConfiguration.parse(
                PREPARED_CONFIGURATION
                    .replace("\"0.0.0.0\"", "\"2001:db8::\"")
                    .replace("\"1.1.1.1\"", "\"2001:4860:4860::8888\"")
                    .replace("\"ipv6\": false", "\"ipv6\": true"),
            ),
        )
        assertEquals("2001:db8::", ipv6.configuration.routes.single().address)
        assertEquals(listOf("2001:4860:4860::8888"), ipv6.configuration.dnsServers)

        assertEquals(
            null,
            NativePreparedVpnConfiguration.parse(
                PREPARED_CONFIGURATION.replace("\"1.1.1.1\"", "\"localhost\""),
            ),
        )
        assertEquals(
            null,
            NativePreparedVpnConfiguration.parse(
                PREPARED_CONFIGURATION.replace("\"1.1.1.1\"", "\"1.1.1.999\""),
            ),
        )
    }

    @Test
    fun preparedValueIsConsumedOnceAndOlderGenerationCannotConsumeNewerValue() {
        val store = VpnPreparedConfigurationStore()
        val prepared = requireNotNull(NativePreparedVpnConfiguration.parse(PREPARED_CONFIGURATION))
        assertEquals(VpnPreparedConfigurationInstallOutcome.Installed, store.install(prepared))
        assertEquals(
            VpnPreparedConfigurationTakeOutcome.TransactionMismatch,
            store.take(8uL, 22uL),
        )
        assertEquals(
            VpnPreparedConfigurationTakeOutcome.StaleGeneration,
            store.take(7uL, 21uL),
        )
        val taken = store.take(8uL, 21uL)
        assertEquals(VpnPreparedConfigurationTakeOutcome.Taken(prepared), taken)
        assertEquals(
            VpnPreparedConfigurationTakeOutcome.AlreadyConsumed,
            store.take(8uL, 21uL),
        )
    }

    @Test
    fun failedStartStopOrRevokeClearsOnlyItsGeneration() {
        val store = VpnPreparedConfigurationStore()
        val first = requireNotNull(NativePreparedVpnConfiguration.parse(PREPARED_CONFIGURATION))
        assertEquals(VpnPreparedConfigurationInstallOutcome.Installed, store.install(first))
        assertEquals(VpnOwnerDataRevision.fromWire(44uL), first.ownerDataRevision)
        assertEquals(VpnPreparedConfigurationClearOutcome.Cleared, store.clearThrough(8uL))
        assertEquals(VpnPreparedConfigurationTakeOutcome.Missing, store.take(8uL, 21uL))

        val newer = first.copy(preparationId = 22uL, vpnGeneration = 9uL)
        assertEquals(VpnPreparedConfigurationInstallOutcome.Installed, store.install(newer))
        assertEquals(
            VpnPreparedConfigurationClearOutcome.PreservedNewer,
            store.clearThrough(8uL),
        )
        assertEquals(
            VpnPreparedConfigurationInstallOutcome.RejectedCurrent,
            store.install(first),
        )
        assertEquals(VpnPreparedConfigurationClearOutcome.Cleared, store.clearThrough(9uL))
    }

    private companion object {
        const val PREPARED_CONFIGURATION = """
            {
              "preparation_id": 21,
              "vpn_generation": 8,
              "owner_data_revision": 44,
              "core_scope": {
                "engine_id": "mihomo",
                "instance_id": "primary",
                "version": "1.19.12",
                "generation": 17,
                "session_token": "340282366920938463463374607431768211455"
              },
              "configuration": {
                "proxy_endpoint": "socks5://127.0.0.1:7891",
                "mtu": 1420,
                "routes": [{"address":"0.0.0.0","prefix":0,"exclude":false}],
                "dns_servers": ["1.1.1.1"],
                "ipv6": false,
                "foreground_requested": true
              }
            }
        """
    }
}

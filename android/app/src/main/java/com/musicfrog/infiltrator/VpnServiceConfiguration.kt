package com.musicfrog.infiltrator

import android.net.InetAddresses
import android.net.IpPrefix
import android.net.VpnService
import android.os.Build
import infiltrator_android.AndroidAppRoutingMode
import infiltrator_android.FfiErrorCode
import infiltrator_android.androidVpnRoutingSnapshot
import org.json.JSONObject
import java.net.InetAddress

internal data class NativeVpnRoute(
    val address: String,
    val prefix: Int,
    val exclude: Boolean,
)

internal data class NativeVpnConfiguration(
    val proxyEndpoint: String,
    val mtu: Int,
    val routes: List<NativeVpnRoute>,
    val dnsServers: List<String>,
    val ipv6: Boolean,
)

internal data class NativeVpnCoreScope(
    val engineId: String,
    val instanceId: String,
    val version: String?,
    val generation: ULong,
    val sessionToken: String,
)

@JvmInline
internal value class VpnOwnerDataRevision(val value: ULong) {
    init {
        require(value != 0uL)
    }

    companion object {
        fun fromWire(value: ULong): VpnOwnerDataRevision? =
            if (value == 0uL) null else VpnOwnerDataRevision(value)
    }
}

internal data class NativePreparedVpnConfiguration(
    val preparationId: ULong,
    val vpnGeneration: ULong,
    val ownerDataRevision: VpnOwnerDataRevision,
    val coreScope: NativeVpnCoreScope,
    val configuration: NativeVpnConfiguration,
) {
    companion object {
        fun parse(raw: String): NativePreparedVpnConfiguration? = try {
            val json = JSONObject(raw)
            if (!json.hasOnly("preparation_id", "vpn_generation", "owner_data_revision", "core_scope", "configuration")) {
                return null
            }
            val preparationId = json.unsigned("preparation_id") ?: return null
            val vpnGeneration = json.unsigned("vpn_generation") ?: return null
            val ownerDataRevision = VpnOwnerDataRevision.fromWire(
                json.unsigned("owner_data_revision") ?: return null,
            ) ?: return null
            if (preparationId == 0uL || vpnGeneration == 0uL) return null

            val scopeJson = json.getJSONObject("core_scope")
            if (!scopeJson.hasOnly("engine_id", "instance_id", "version", "generation", "session_token")) {
                return null
            }
            val engineId = scopeJson.getString("engine_id").trim()
            val instanceId = scopeJson.getString("instance_id").trim()
            val version = if (scopeJson.isNull("version")) null else scopeJson.getString("version").trim()
            val coreGeneration = scopeJson.unsigned("generation") ?: return null
            val sessionToken = scopeJson.getString("session_token")
            if (
                !engineId.matches(Regex("[a-z0-9_-]{1,64}")) ||
                instanceId.isEmpty() || instanceId.length > 128 ||
                version?.isEmpty() == true || coreGeneration == 0uL ||
                !isValidSessionToken(sessionToken)
            ) {
                return null
            }

            val configurationJson = json.getJSONObject("configuration")
            val configuration = parseBuilderVpnConfiguration(configurationJson)
                ?: return null
            NativePreparedVpnConfiguration(
                preparationId,
                vpnGeneration,
                ownerDataRevision,
                NativeVpnCoreScope(engineId, instanceId, version, coreGeneration, sessionToken),
                configuration,
            )
        } catch (_: Exception) {
            null
        }
    }
}

private fun parseBuilderVpnConfiguration(json: JSONObject): NativeVpnConfiguration? = try {
    if (!json.hasOnly("proxy_endpoint", "mtu", "routes", "dns_servers", "ipv6", "foreground_requested")) {
        return null
    }
    val proxyEndpoint = json.getString("proxy_endpoint").trim()
    val mtu = json.getInt("mtu")
    val ipv6 = json.getBoolean("ipv6")
    if (proxyEndpoint.isEmpty() || mtu !in 1280..9000 || !json.getBoolean("foreground_requested")) {
        return null
    }

    val routeArray = json.getJSONArray("routes")
    if (routeArray.length() == 0) return null
    val routes = buildList(routeArray.length()) {
        for (index in 0 until routeArray.length()) {
            val route = routeArray.getJSONObject(index)
            if (!route.hasOnly("address", "prefix", "exclude")) return null
            val address = route.getString("address").trim()
            val prefix = route.getInt("prefix")
            val addressBits = parseNumericAddress(address)?.address?.size?.times(8) ?: return null
            if (address.isEmpty() || prefix !in 0..addressBits) return null
            add(
                NativeVpnRoute(
                    address = address,
                    prefix = prefix,
                    exclude = route.getBoolean("exclude"),
                ),
            )
        }
    }

    val dnsArray = json.getJSONArray("dns_servers")
    val dnsServers = buildList(dnsArray.length()) {
        for (index in 0 until dnsArray.length()) {
            val address = dnsArray.getString(index).trim()
            val parsedAddress = parseNumericAddress(address) ?: return null
            if (!ipv6 && parsedAddress.address.size == 16) return null
            add(address)
        }
    }
    NativeVpnConfiguration(proxyEndpoint, mtu, routes, dnsServers, ipv6)
} catch (_: Exception) {
    null
}

private fun parseNumericAddress(value: String): InetAddress? = try {
    if (value.isEmpty()) null else InetAddresses.parseNumericAddress(value)
} catch (_: IllegalArgumentException) {
    null
}

private fun JSONObject.hasOnly(vararg expected: String): Boolean =
    keys().asSequence().toSet() == expected.toSet()

private fun JSONObject.unsigned(name: String): ULong? =
    opt(name)?.toString()?.toULongOrNull()

private fun isValidSessionToken(value: String): Boolean {
    if (!value.matches(Regex("[1-9][0-9]{0,38}"))) return false
    return try {
        java.math.BigInteger(value) <= java.math.BigInteger("340282366920938463463374607431768211455")
    } catch (_: NumberFormatException) {
        false
    }
}

internal sealed interface VpnPreparedConfigurationInstallOutcome {
    data object Installed : VpnPreparedConfigurationInstallOutcome

    data object ReplacedOlder : VpnPreparedConfigurationInstallOutcome

    data object RejectedCurrent : VpnPreparedConfigurationInstallOutcome
}

internal sealed interface VpnPreparedConfigurationTakeOutcome {
    data class Taken(val configuration: NativePreparedVpnConfiguration) : VpnPreparedConfigurationTakeOutcome

    data object Missing : VpnPreparedConfigurationTakeOutcome

    data object NotReady : VpnPreparedConfigurationTakeOutcome

    data object TransactionMismatch : VpnPreparedConfigurationTakeOutcome

    data object StaleGeneration : VpnPreparedConfigurationTakeOutcome

    data object AlreadyConsumed : VpnPreparedConfigurationTakeOutcome
}

internal sealed interface VpnPreparedConfigurationClearOutcome {
    data object Cleared : VpnPreparedConfigurationClearOutcome

    data object NotRequired : VpnPreparedConfigurationClearOutcome

    data object PreservedNewer : VpnPreparedConfigurationClearOutcome
}

internal class VpnPreparedConfigurationStore {
    private sealed interface State {
        data object Empty : State

        data class Ready(val configuration: NativePreparedVpnConfiguration) : State

        data class Consumed(val preparationId: ULong, val vpnGeneration: ULong) : State
    }

    private var state: State = State.Empty

    @Synchronized
    fun install(configuration: NativePreparedVpnConfiguration): VpnPreparedConfigurationInstallOutcome =
        when (val current = state) {
            State.Empty -> {
                state = State.Ready(configuration)
                VpnPreparedConfigurationInstallOutcome.Installed
            }
            is State.Ready -> if (current.configuration.vpnGeneration < configuration.vpnGeneration) {
                state = State.Ready(configuration)
                VpnPreparedConfigurationInstallOutcome.ReplacedOlder
            } else {
                VpnPreparedConfigurationInstallOutcome.RejectedCurrent
            }
            is State.Consumed -> if (current.vpnGeneration < configuration.vpnGeneration) {
                state = State.Ready(configuration)
                VpnPreparedConfigurationInstallOutcome.ReplacedOlder
            } else {
                VpnPreparedConfigurationInstallOutcome.RejectedCurrent
            }
        }

    @Synchronized
    fun take(vpnGeneration: ULong, preparationId: ULong): VpnPreparedConfigurationTakeOutcome =
        when (val current = state) {
            State.Empty -> VpnPreparedConfigurationTakeOutcome.Missing
            is State.Ready -> {
                val prepared = current.configuration
                when {
                    vpnGeneration != prepared.vpnGeneration -> {
                        if (vpnGeneration > prepared.vpnGeneration) state = State.Empty
                        VpnPreparedConfigurationTakeOutcome.StaleGeneration
                    }
                    preparationId != prepared.preparationId ->
                        VpnPreparedConfigurationTakeOutcome.TransactionMismatch
                    else -> {
                        state = State.Consumed(preparationId, vpnGeneration)
                        VpnPreparedConfigurationTakeOutcome.Taken(prepared)
                    }
                }
            }
            is State.Consumed -> when {
                current.vpnGeneration == vpnGeneration && current.preparationId == preparationId ->
                    VpnPreparedConfigurationTakeOutcome.AlreadyConsumed
                current.vpnGeneration < vpnGeneration -> {
                    state = State.Empty
                    VpnPreparedConfigurationTakeOutcome.StaleGeneration
                }
                else -> VpnPreparedConfigurationTakeOutcome.Missing
            }
        }

    @Synchronized
    fun clearThrough(vpnGeneration: ULong): VpnPreparedConfigurationClearOutcome =
        when (val current = state) {
            State.Empty -> VpnPreparedConfigurationClearOutcome.NotRequired
            is State.Ready -> if (current.configuration.vpnGeneration <= vpnGeneration) {
                state = State.Empty
                VpnPreparedConfigurationClearOutcome.Cleared
            } else {
                VpnPreparedConfigurationClearOutcome.PreservedNewer
            }
            is State.Consumed -> if (current.vpnGeneration <= vpnGeneration) {
                state = State.Empty
                VpnPreparedConfigurationClearOutcome.Cleared
            } else {
                VpnPreparedConfigurationClearOutcome.PreservedNewer
            }
        }
}

internal sealed interface VpnRoutingConfiguration {
    data class Ready(
        val mode: AndroidAppRoutingMode,
        val packages: Set<String>,
        val routingRevision: ULong,
    ) : VpnRoutingConfiguration

    data class Failed(val message: String) : VpnRoutingConfiguration
}

internal sealed interface VpnPackageRule {
    data class Allow(val packageName: String) : VpnPackageRule

    data class Disallow(val packageName: String) : VpnPackageRule
}

internal sealed interface VpnPackageRoutingPlan {
    data class Ready(val rules: List<VpnPackageRule>) : VpnPackageRoutingPlan

    data class Failed(val message: String) : VpnPackageRoutingPlan
}

internal fun loadVpnRoutingConfiguration(): VpnRoutingConfiguration = try {
    val ownerSnapshot = androidVpnRoutingSnapshot()
    val result = ownerSnapshot.result
    val config = result.snapshot
    if (result.status.code != FfiErrorCode.OK || config == null) {
        VpnRoutingConfiguration.Failed(
            result.status.message ?: "VPN app-routing configuration is unavailable",
        )
    } else {
        VpnRoutingConfiguration.Ready(
            config.mode,
            config.packages.toSet(),
            ownerSnapshot.routingRevision,
        )
    }
} catch (error: Exception) {
    VpnRoutingConfiguration.Failed(error.message ?: "VPN app-routing configuration could not be loaded")
}

internal fun buildVpnPackageRoutingPlan(
    configuration: VpnRoutingConfiguration,
    ownPackageName: String,
): VpnPackageRoutingPlan = when (configuration) {
    is VpnRoutingConfiguration.Failed -> VpnPackageRoutingPlan.Failed(configuration.message)
    is VpnRoutingConfiguration.Ready -> {
        if (configuration.packages.any(String::isBlank)) {
            return VpnPackageRoutingPlan.Failed("VPN app-routing configuration contains an empty package name")
        }
        val selected = configuration.packages
            .toSortedSet()
        when (configuration.mode) {
            AndroidAppRoutingMode.PROXY_ALL -> VpnPackageRoutingPlan.Ready(
                listOf(VpnPackageRule.Disallow(ownPackageName)),
            )
            AndroidAppRoutingMode.PROXY_SELECTED -> {
                val packages = selected.filterNot { it == ownPackageName }
                if (packages.isEmpty()) {
                    VpnPackageRoutingPlan.Failed("Select at least one app to use selected-app routing")
                } else {
                    VpnPackageRoutingPlan.Ready(packages.map(VpnPackageRule::Allow))
                }
            }
            AndroidAppRoutingMode.BYPASS_SELECTED -> VpnPackageRoutingPlan.Ready(
                buildList {
                    add(VpnPackageRule.Disallow(ownPackageName))
                    selected.filterNot { it == ownPackageName }
                        .forEach { add(VpnPackageRule.Disallow(it)) }
                },
            )
        }
    }
}

@Suppress("NewApi")
internal fun applyVpnRoute(builder: VpnService.Builder, route: NativeVpnRoute) {
    val address = InetAddresses.parseNumericAddress(route.address)
    if (route.exclude) {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU) {
            throw IllegalArgumentException("VPN excluded routes require Android API 33 or newer")
        }
        builder.excludeRoute(IpPrefix(address, route.prefix))
    } else {
        builder.addRoute(address, route.prefix)
    }
}

internal fun applyVpnPackageRouting(builder: VpnService.Builder, plan: VpnPackageRoutingPlan) {
    when (plan) {
        is VpnPackageRoutingPlan.Failed -> throw IllegalArgumentException(plan.message)
        is VpnPackageRoutingPlan.Ready -> for (rule in plan.rules) {
            when (rule) {
                is VpnPackageRule.Allow -> builder.addAllowedApplication(rule.packageName)
                is VpnPackageRule.Disallow -> builder.addDisallowedApplication(rule.packageName)
            }
        }
    }
}

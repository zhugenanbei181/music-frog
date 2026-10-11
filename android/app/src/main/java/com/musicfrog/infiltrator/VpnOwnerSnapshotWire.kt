package com.musicfrog.infiltrator

import org.json.JSONArray
import org.json.JSONObject
import infiltrator_android.AndroidCoreLifecycle

internal sealed interface VpnOwnerLifecycleWire {
    data object PermissionRequired : VpnOwnerLifecycleWire
    data object StartRequested : VpnOwnerLifecycleWire
    data object Starting : VpnOwnerLifecycleWire
    data object Foreground : VpnOwnerLifecycleWire
    data object TunnelEstablished : VpnOwnerLifecycleWire
    data object Running : VpnOwnerLifecycleWire
    data object Stopping : VpnOwnerLifecycleWire
    data object Stopped : VpnOwnerLifecycleWire
    data object Revoked : VpnOwnerLifecycleWire
    data class Failed(val failureMessage: String) : VpnOwnerLifecycleWire
    data class Unavailable(val failureMessage: String) : VpnOwnerLifecycleWire

    fun toJsonValue(): Any = when (this) {
        PermissionRequired -> "PermissionRequired"
        StartRequested -> "StartRequested"
        Starting -> "Starting"
        Foreground -> "Foreground"
        TunnelEstablished -> "TunnelEstablished"
        Running -> "Running"
        Stopping -> "Stopping"
        Stopped -> "Stopped"
        Revoked -> "Revoked"
        is Failed -> JSONObject().put("Failed", JSONObject().put("failure_message", failureMessage))
        is Unavailable -> JSONObject().put("Unavailable", JSONObject().put("failure_message", failureMessage))
    }
}

internal enum class VpnOwnerReadinessWire(val wireName: String) {
    Unavailable("unavailable"),
    PermissionRequired("permission_required"),
    AwaitingForeground("awaiting_foreground"),
    AwaitingTunnel("awaiting_tunnel"),
    AwaitingDataPlane("awaiting_data_plane"),
    Ready("ready"),
    Stopping("stopping"),
    CleanupPending("cleanup_pending"),
    Stopped("stopped"),
    Revoked("revoked"),
    Failed("failed"),
}

internal enum class VpnOwnerResourceKindWire(val wireName: String) {
    PreparedConfiguration("prepared_configuration"),
    Worker("worker"),
    TunnelDescriptor("tunnel_descriptor"),
    RoutingLease("routing_lease"),
}

internal sealed interface VpnOwnerResourceStateWire {
    data object Absent : VpnOwnerResourceStateWire

    data class Held(val generation: Long) : VpnOwnerResourceStateWire

    data class CleanupPending(val generation: Long, val failureMessage: String) : VpnOwnerResourceStateWire
}

internal data class VpnOwnerResourceFactWire(
    val kind: VpnOwnerResourceKindWire,
    val state: VpnOwnerResourceStateWire,
)

internal sealed interface VpnOwnerCoreReadbackWire {
    data class Available(val snapshot: CoreOwnerSnapshotWire) : VpnOwnerCoreReadbackWire

    data class Unavailable(val failureMessage: String) : VpnOwnerCoreReadbackWire
}

internal sealed interface CoreOwnerIdentityWire {
    data object Absent : CoreOwnerIdentityWire

    data class Present(
        val engineId: String,
        val instanceId: String,
        val version: String?,
    ) : CoreOwnerIdentityWire {
        init {
            require(engineId.isNotBlank() && instanceId.isNotBlank())
            require(version == null || version.isNotBlank())
        }
    }

    fun toJsonValue(): Any = when (this) {
        Absent -> JSONObject.NULL
        is Present -> JSONObject()
            .put("engine", engineId)
            .put("instance", instanceId)
            .put("version", version ?: JSONObject.NULL)
    }

    companion object {
        fun parse(raw: String): CoreOwnerIdentityWire? = runCatching {
            if (raw == "null") {
                Absent
            } else {
                val json = JSONObject(raw)
                require(json.keys().asSequence().toSet() == setOf("engine", "instance", "version"))
                Present(
                    engineId = json.getString("engine"),
                    instanceId = json.getString("instance"),
                    version = if (json.isNull("version")) null else json.getString("version"),
                )
            }
        }.getOrNull()

        fun fromNullableParts(
            engineId: String?,
            instanceId: String?,
            version: String?,
        ): CoreOwnerIdentityWire? = when {
            engineId == null && instanceId == null && version == null -> Absent
            engineId != null && instanceId != null -> Present(engineId, instanceId, version)
            else -> null
        }
    }
}

internal enum class CoreOwnerLifecycleWire(val wireName: String) {
    Stopped("Stopped"),
    Starting("Starting"),
    Ready("Ready"),
    Running("Running"),
    Stopping("Stopping"),
    Failed("Failed"),
}

internal fun AndroidCoreLifecycle.toCoreOwnerLifecycleWire(): CoreOwnerLifecycleWire = when (this) {
    AndroidCoreLifecycle.STOPPED -> CoreOwnerLifecycleWire.Stopped
    AndroidCoreLifecycle.STARTING -> CoreOwnerLifecycleWire.Starting
    AndroidCoreLifecycle.READY -> CoreOwnerLifecycleWire.Ready
    AndroidCoreLifecycle.RUNNING -> CoreOwnerLifecycleWire.Running
    AndroidCoreLifecycle.STOPPING -> CoreOwnerLifecycleWire.Stopping
    AndroidCoreLifecycle.FAILED -> CoreOwnerLifecycleWire.Failed
}

internal fun CoreOwnerLifecycleWire.toAndroidCoreLifecycle(): AndroidCoreLifecycle = when (this) {
    CoreOwnerLifecycleWire.Stopped -> AndroidCoreLifecycle.STOPPED
    CoreOwnerLifecycleWire.Starting -> AndroidCoreLifecycle.STARTING
    CoreOwnerLifecycleWire.Ready -> AndroidCoreLifecycle.READY
    CoreOwnerLifecycleWire.Running -> AndroidCoreLifecycle.RUNNING
    CoreOwnerLifecycleWire.Stopping -> AndroidCoreLifecycle.STOPPING
    CoreOwnerLifecycleWire.Failed -> AndroidCoreLifecycle.FAILED
}

internal enum class CoreOwnerFailureCodeWire(val wireName: String) {
    InvalidInput("InvalidInput"),
    InvalidState("InvalidState"),
    NotReady("NotReady"),
    Unsupported("Unsupported"),
    Network("Network"),
    Authentication("Authentication"),
    Configuration("Configuration"),
    Storage("Storage"),
    Permission("Permission"),
    Canceled("Canceled"),
    Internal("Internal"),
}

internal sealed interface CoreOwnerFailureReasonWire {
    data class YamlSyntax(val line: Long, val column: Long) : CoreOwnerFailureReasonWire
    data object MixinYaml : CoreOwnerFailureReasonWire
    data object ActiveProfileInconsistent : CoreOwnerFailureReasonWire
    data object QuotaSourceChanged : CoreOwnerFailureReasonWire
    data object CurrentTimeUnavailable : CoreOwnerFailureReasonWire
    data object DownloadCanceled : CoreOwnerFailureReasonWire

    fun toJsonValue(): Any = when (this) {
        is YamlSyntax -> JSONObject().put(
            "YamlSyntax",
            JSONObject().put("line", line).put("column", column),
        )
        MixinYaml -> "MixinYaml"
        ActiveProfileInconsistent -> "ActiveProfileInconsistent"
        QuotaSourceChanged -> "QuotaSourceChanged"
        CurrentTimeUnavailable -> "CurrentTimeUnavailable"
        DownloadCanceled -> "DownloadCanceled"
    }

    companion object {
        fun parse(value: Any): CoreOwnerFailureReasonWire = when (value) {
            is String -> when (value) {
                "MixinYaml" -> MixinYaml
                "ActiveProfileInconsistent" -> ActiveProfileInconsistent
                "QuotaSourceChanged" -> QuotaSourceChanged
                "CurrentTimeUnavailable" -> CurrentTimeUnavailable
                "DownloadCanceled" -> DownloadCanceled
                else -> error("unknown Core failure reason")
            }
            is JSONObject -> {
                require(value.length() == 1 && value.has("YamlSyntax"))
                val syntax = value.getJSONObject("YamlSyntax")
                require(syntax.keys().asSequence().toSet() == setOf("line", "column"))
                YamlSyntax(
                    syntax.getLong("line").also { require(it >= 0L) },
                    syntax.getLong("column").also { require(it >= 0L) },
                )
            }
            else -> error("invalid Core failure reason")
        }
    }
}

internal data class CoreOwnerFailureWire(
    val code: CoreOwnerFailureCodeWire,
    val message: String,
    val retryable: Boolean,
    val reason: CoreOwnerFailureReasonWire?,
) {
    fun toJson(): JSONObject = JSONObject()
        .put("code", code.wireName)
        .put("message", message)
        .put("retryable", retryable)
        .also { json -> reason?.let { json.put("reason", it.toJsonValue()) } }

    companion object {
        fun parse(raw: String): CoreOwnerFailureWire? = runCatching {
            parse(JSONObject(raw))
        }.getOrNull()

        fun parse(json: JSONObject): CoreOwnerFailureWire {
            val keys = json.keys().asSequence().toSet()
            require(keys == setOf("code", "message", "retryable") ||
                keys == setOf("code", "message", "retryable", "reason"))
            val reason = if (json.has("reason")) CoreOwnerFailureReasonWire.parse(json.get("reason")) else null
            CoreOwnerFailureWire(
                code = CoreOwnerFailureCodeWire.entries.first { it.wireName == json.getString("code") },
                message = json.getString("message"),
                retryable = json.getBoolean("retryable"),
                reason = reason,
            )
        }
    }
}

internal data class CoreOwnerSnapshotWire(
    val identity: CoreOwnerIdentityWire,
    val lifecycle: CoreOwnerLifecycleWire,
    val generation: Long,
    val sessionToken: String?,
    val revision: Long,
    val failure: CoreOwnerFailureWire?,
) {
    init {
        require(generation >= 0L && revision >= 0L)
        require(sessionToken == null || sessionToken.isNotBlank() && sessionToken.all(Char::isDigit))
    }

    fun toJson(): JSONObject = JSONObject()
        .put("identity", identity.toJsonValue())
        .put("lifecycle", lifecycle.wireName)
        .put("generation", generation)
        .put("session_token", sessionToken ?: JSONObject.NULL)
        .put("revision", revision)
        .put("failure", failure?.toJson() ?: JSONObject.NULL)
}

internal data class VpnOwnerSnapshotFacts(
    val ownerSessionId: String,
    val vpnGeneration: Long,
    val vpnState: VpnOwnerLifecycleWire,
    val readiness: VpnOwnerReadinessWire,
    val resources: List<VpnOwnerResourceFactWire>,
    val foreground: Boolean,
    val mtu: Int?,
    val routeCount: Int,
    val dnsServers: List<String>,
    val ipv6: Boolean,
    val core: VpnOwnerCoreReadbackWire,
)

internal sealed interface VpnOwnerDataPlaneState {
    data object PermissionRequired : VpnOwnerDataPlaneState
    data object Starting : VpnOwnerDataPlaneState
    data object Running : VpnOwnerDataPlaneState
    data object Stopping : VpnOwnerDataPlaneState
    data class CleanupPending(val failureMessage: String) : VpnOwnerDataPlaneState
    data object Stopped : VpnOwnerDataPlaneState
    data object Revoked : VpnOwnerDataPlaneState
    data class Failed(val failureMessage: String) : VpnOwnerDataPlaneState
    data class Unavailable(val failureMessage: String) : VpnOwnerDataPlaneState
}

internal data class VpnOwnerSnapshotWire(
    val ownerSessionId: String,
    val revision: Long,
    val vpnGeneration: Long,
    val vpnState: VpnOwnerLifecycleWire,
    val readiness: VpnOwnerReadinessWire,
    val resources: List<VpnOwnerResourceFactWire>,
    val foreground: Boolean,
    val mtu: Int?,
    val routeCount: Int,
    val dnsServers: List<String>,
    val ipv6: Boolean,
    val core: VpnOwnerCoreReadbackWire,
    val canonicalJson: String,
) {
    fun dataPlaneState(): VpnOwnerDataPlaneState {
        resources.firstNotNullOfOrNull { fact ->
            (fact.state as? VpnOwnerResourceStateWire.CleanupPending)
                ?.let { pending -> VpnOwnerDataPlaneState.CleanupPending(pending.failureMessage) }
        }?.let { return it }

        return when (vpnState) {
            is VpnOwnerLifecycleWire.Unavailable ->
                VpnOwnerDataPlaneState.Unavailable(vpnState.failureMessage)
            is VpnOwnerLifecycleWire.Failed ->
                VpnOwnerDataPlaneState.Failed(vpnState.failureMessage)
            else -> when (readiness) {
                VpnOwnerReadinessWire.Unavailable ->
                    VpnOwnerDataPlaneState.Unavailable("VPN owner readback is unavailable")
                VpnOwnerReadinessWire.Failed ->
                    VpnOwnerDataPlaneState.Failed("VPN owner published failed readiness")
                VpnOwnerReadinessWire.CleanupPending ->
                    VpnOwnerDataPlaneState.CleanupPending("VPN owner cleanup is pending")
                VpnOwnerReadinessWire.PermissionRequired -> VpnOwnerDataPlaneState.PermissionRequired
                VpnOwnerReadinessWire.AwaitingForeground,
                VpnOwnerReadinessWire.AwaitingTunnel,
                VpnOwnerReadinessWire.AwaitingDataPlane -> VpnOwnerDataPlaneState.Starting
                VpnOwnerReadinessWire.Stopping -> VpnOwnerDataPlaneState.Stopping
                VpnOwnerReadinessWire.Ready -> when (vpnState) {
                    VpnOwnerLifecycleWire.Running -> if (isRunningDataPlane()) {
                        VpnOwnerDataPlaneState.Running
                    } else if (!foreground) {
                        VpnOwnerDataPlaneState.Failed(
                            "VPN Running readback lacks foreground protection",
                        )
                    } else {
                        VpnOwnerDataPlaneState.Failed(
                            "VPN Running readback lacks its current worker or tunnel resource",
                        )
                    }
                    VpnOwnerLifecycleWire.PermissionRequired -> VpnOwnerDataPlaneState.PermissionRequired
                    VpnOwnerLifecycleWire.StartRequested,
                    VpnOwnerLifecycleWire.Starting,
                    VpnOwnerLifecycleWire.Foreground,
                    VpnOwnerLifecycleWire.TunnelEstablished -> VpnOwnerDataPlaneState.Starting
                    VpnOwnerLifecycleWire.Stopping -> VpnOwnerDataPlaneState.Stopping
                    VpnOwnerLifecycleWire.Stopped,
                    VpnOwnerLifecycleWire.Revoked -> VpnOwnerDataPlaneState.Failed(
                        "VPN terminal lifecycle conflicts with Ready readiness",
                    )
                    is VpnOwnerLifecycleWire.Failed,
                    is VpnOwnerLifecycleWire.Unavailable -> error("handled above")
                }
                VpnOwnerReadinessWire.Stopped -> if (
                    vpnState == VpnOwnerLifecycleWire.Stopped && isTerminalStopped()
                ) {
                    VpnOwnerDataPlaneState.Stopped
                } else {
                    VpnOwnerDataPlaneState.Failed(
                        "VPN Stopped readback still owns a resource or conflicts with lifecycle",
                    )
                }
                VpnOwnerReadinessWire.Revoked -> if (
                    vpnState == VpnOwnerLifecycleWire.Revoked && isTerminalStopped()
                ) {
                    VpnOwnerDataPlaneState.Revoked
                } else {
                    VpnOwnerDataPlaneState.Failed(
                        "VPN Revoked readback still owns a resource or conflicts with lifecycle",
                    )
                }
            }
        }
    }

    fun isRunningDataPlane(): Boolean =
        vpnState == VpnOwnerLifecycleWire.Running &&
            readiness == VpnOwnerReadinessWire.Ready &&
            foreground &&
            resourceHeld(VpnOwnerResourceKindWire.Worker) &&
            resourceHeld(VpnOwnerResourceKindWire.TunnelDescriptor)

    private fun resourceHeld(kind: VpnOwnerResourceKindWire): Boolean =
        resources.singleOrNull { it.kind == kind }?.state ==
            VpnOwnerResourceStateWire.Held(vpnGeneration)

    fun isTerminalStopped(): Boolean {
        val terminalReadiness = when (vpnState) {
            VpnOwnerLifecycleWire.Stopped -> readiness == VpnOwnerReadinessWire.Stopped
            VpnOwnerLifecycleWire.Revoked -> readiness == VpnOwnerReadinessWire.Revoked
            else -> false
        }
        return terminalReadiness && resourcesReleased()
    }

    fun resourcesReleased(): Boolean =
        resources.size == VpnOwnerResourceKindWire.entries.size &&
            resources.map { it.kind }.toSet() == VpnOwnerResourceKindWire.entries.toSet() &&
            resources.all { it.state == VpnOwnerResourceStateWire.Absent }

    fun coreLifecycleJson(): String? = when (val readback = core) {
        is VpnOwnerCoreReadbackWire.Unavailable -> null
        is VpnOwnerCoreReadbackWire.Available -> readback.snapshot.toJson().toString()
    }

    companion object {
        fun parse(raw: String): VpnOwnerSnapshotWire? = runCatching {
            val json = JSONObject(raw)
            require(json.keys().asSequence().toSet() == setOf(
                "owner_session_id", "revision", "vpn_generation", "vpn_state", "readiness",
                "resources", "foreground", "mtu", "route_count", "dns_servers", "ipv6", "core",
            ))
            val ownerSessionId = json.getString("owner_session_id").trim().also { require(it.isNotEmpty()) }
            val revision = json.getLong("revision").also { require(it > 0L) }
            val vpnGeneration = json.getLong("vpn_generation").also { require(it >= 0L) }
            val resourcesJson = json.getJSONArray("resources")
            val resources = buildList(resourcesJson.length()) {
                for (index in 0 until resourcesJson.length()) {
                    val resource = resourcesJson.getJSONObject(index)
                    require(resource.keys().asSequence().toSet() == setOf("kind", "state"))
                    val kindName = resource.getString("kind")
                    val kind = VpnOwnerResourceKindWire.entries.first { it.wireName == kindName }
                    val stateJson = resource.get("state")
                    val state = when (stateJson) {
                        "Absent" -> VpnOwnerResourceStateWire.Absent
                        else -> {
                            val tagged = stateJson as JSONObject
                            require(tagged.length() == 1)
                            when (tagged.keys().next()) {
                                "Held" -> {
                                    val held = tagged.getJSONObject("Held")
                                    require(held.keys().asSequence().toSet() == setOf("generation"))
                                    VpnOwnerResourceStateWire.Held(held.getLong("generation").also { require(it >= 0L) })
                                }
                                "CleanupPending" -> {
                                    val pending = tagged.getJSONObject("CleanupPending")
                                    require(pending.keys().asSequence().toSet() == setOf("generation", "failure_message"))
                                    VpnOwnerResourceStateWire.CleanupPending(
                                        pending.getLong("generation").also { require(it >= 0L) },
                                        pending.getString("failure_message").also { require(it.isNotBlank()) },
                                    )
                                }
                                else -> error("unknown resource state")
                            }
                        }
                    }
                    add(VpnOwnerResourceFactWire(kind, state))
                }
            }
            require(resources.map { it.kind }.toSet() == VpnOwnerResourceKindWire.entries.toSet())
            val coreJson = json.getJSONObject("core")
            val core = when {
                coreJson.has("Available") && coreJson.length() == 1 -> {
                    val coreSnapshot = coreJson.getJSONObject("Available")
                    require(coreSnapshot.keys().asSequence().toSet() == setOf(
                        "identity", "lifecycle", "generation", "session_token", "revision", "failure",
                    ))
                    val identityJson = coreSnapshot.get("identity")
                    val identity = when (identityJson) {
                        JSONObject.NULL -> CoreOwnerIdentityWire.Absent
                        is JSONObject -> {
                            require(identityJson.keys().asSequence().toSet() == setOf("engine", "instance", "version"))
                            CoreOwnerIdentityWire.Present(
                                engineId = identityJson.getString("engine"),
                                instanceId = identityJson.getString("instance"),
                                version = if (identityJson.isNull("version")) null else identityJson.getString("version"),
                            )
                        }
                        else -> error("invalid Core identity")
                    }
                    val failureJson = coreSnapshot.get("failure")
                    val failure = when (failureJson) {
                        JSONObject.NULL -> null
                        is JSONObject -> CoreOwnerFailureWire.parse(failureJson)
                        else -> error("invalid Core failure")
                    }
                    VpnOwnerCoreReadbackWire.Available(
                        CoreOwnerSnapshotWire(
                            identity = identity,
                            lifecycle = CoreOwnerLifecycleWire.entries.first {
                                it.wireName == coreSnapshot.getString("lifecycle")
                            },
                            generation = coreSnapshot.getLong("generation").also { require(it >= 0L) },
                            sessionToken = coreSnapshot.get("session_token").let {
                                if (it == JSONObject.NULL) null else it.toString().also { token ->
                                    require(token.isNotBlank() && token.all(Char::isDigit))
                                }
                            },
                            revision = coreSnapshot.getLong("revision").also { require(it >= 0L) },
                            failure = failure,
                        ),
                    )
                }
                coreJson.has("Unavailable") && coreJson.length() == 1 -> {
                    val unavailable = coreJson.getJSONObject("Unavailable")
                    require(unavailable.keys().asSequence().toSet() == setOf("failure_message"))
                    VpnOwnerCoreReadbackWire.Unavailable(unavailable.getString("failure_message"))
                }
                else -> error("invalid core readback")
            }
            val state = parseLifecycle(json.get("vpn_state"))
            val readiness = VpnOwnerReadinessWire.entries.first { it.wireName == json.getString("readiness") }
            val mtu = if (json.isNull("mtu")) null else json.getInt("mtu").also { require(it > 0) }
            val dnsJson = json.getJSONArray("dns_servers")
            val dnsServers = buildList(dnsJson.length()) {
                for (index in 0 until dnsJson.length()) add(dnsJson.getString(index))
            }
            VpnOwnerSnapshotWire(
                ownerSessionId = ownerSessionId,
                revision = revision,
                vpnGeneration = vpnGeneration,
                vpnState = state,
                readiness = readiness,
                resources = resources,
                foreground = json.getBoolean("foreground"),
                mtu = mtu,
                routeCount = json.getInt("route_count").also { require(it >= 0) },
                dnsServers = dnsServers,
                ipv6 = json.getBoolean("ipv6"),
                core = core,
                canonicalJson = json.toString(),
            )
        }.getOrNull()

        private fun parseLifecycle(value: Any): VpnOwnerLifecycleWire = when (value) {
            "PermissionRequired" -> VpnOwnerLifecycleWire.PermissionRequired
            "StartRequested" -> VpnOwnerLifecycleWire.StartRequested
            "Starting" -> VpnOwnerLifecycleWire.Starting
            "Foreground" -> VpnOwnerLifecycleWire.Foreground
            "TunnelEstablished" -> VpnOwnerLifecycleWire.TunnelEstablished
            "Running" -> VpnOwnerLifecycleWire.Running
            "Stopping" -> VpnOwnerLifecycleWire.Stopping
            "Stopped" -> VpnOwnerLifecycleWire.Stopped
            "Revoked" -> VpnOwnerLifecycleWire.Revoked
            else -> {
                val tagged = value as JSONObject
                require(tagged.length() == 1)
                val tag = tagged.keys().next()
                val payload = tagged.getJSONObject(tag)
                require(payload.keys().asSequence().toSet() == setOf("failure_message"))
                when (tag) {
                    "Failed" -> VpnOwnerLifecycleWire.Failed(payload.getString("failure_message"))
                    "Unavailable" -> VpnOwnerLifecycleWire.Unavailable(payload.getString("failure_message"))
                    else -> error("unknown VPN lifecycle state")
                }
            }
        }
    }
}

internal data class VpnOwnerConfigurationWire(
    val mtu: Int,
    val routeCount: Int,
    val dnsServers: List<String>,
    val ipv6: Boolean,
)

/** Holds the authoritative owner revision; identical reads return the same revision. */
internal class VpnOwnerSnapshotPublisher {
    private sealed interface State {
        data object Empty : State

        data class Published(
            val facts: VpnOwnerSnapshotFacts,
            val revision: Long,
        ) : State
    }

    private var state: State = State.Empty

    @Synchronized
    fun publish(facts: VpnOwnerSnapshotFacts): String {
        val published = when (val current = state) {
            State.Empty -> State.Published(facts, 1L)
            is State.Published -> if (current.facts == facts) {
                current
            } else {
                State.Published(facts, current.revision.saturatingIncrement())
            }
        }
        state = published
        return published.toJson()
    }

    private fun State.Published.toJson(): String = JSONObject()
        .put("owner_session_id", facts.ownerSessionId)
        .put("revision", revision)
        .put("vpn_generation", facts.vpnGeneration)
        .put("vpn_state", facts.vpnState.toJsonValue())
        .put("readiness", facts.readiness.wireName)
        .put(
            "resources",
            JSONArray().apply { facts.resources.forEach { put(it.toJson()) } },
        )
        .put("foreground", facts.foreground)
        .put("mtu", facts.mtu ?: JSONObject.NULL)
        .put("route_count", facts.routeCount)
        .put("dns_servers", JSONArray().apply { facts.dnsServers.forEach { address -> put(address) } })
        .put("ipv6", facts.ipv6)
        .put("core", facts.core.toJson())
        .toString()

    private fun VpnOwnerResourceFactWire.toJson(): JSONObject = JSONObject()
        .put("kind", kind.wireName)
        .put("state", state.toJson())

    private fun VpnOwnerResourceStateWire.toJson(): Any = when (this) {
        VpnOwnerResourceStateWire.Absent -> "Absent"
        is VpnOwnerResourceStateWire.Held -> JSONObject().put(
            "Held",
            JSONObject().put("generation", generation),
        )
        is VpnOwnerResourceStateWire.CleanupPending -> JSONObject().put(
            "CleanupPending",
            JSONObject()
                .put("generation", generation)
                .put("failure_message", failureMessage),
        )
    }

    private fun VpnOwnerCoreReadbackWire.toJson(): JSONObject = when (this) {
        is VpnOwnerCoreReadbackWire.Available -> JSONObject().put(
            "Available",
            snapshot.toJson(),
        )
        is VpnOwnerCoreReadbackWire.Unavailable -> JSONObject().put(
            "Unavailable",
            JSONObject().put("failure_message", failureMessage),
        )
    }

}

private fun Long.saturatingIncrement(): Long = if (this == Long.MAX_VALUE) Long.MAX_VALUE else this + 1L

package com.musicfrog.infiltrator.ui.common

import org.json.JSONObject

internal fun parseCoreCapabilitySnapshot(
    data: JSONObject,
    rawDataJson: String,
): CoreCapabilitySnapshotObservation {
    val scopeJson = data.getJSONObject("scope")
    val scope = parseCapabilityScope(
        scopeJson,
        rawJsonValueAtPath(rawDataJson, "scope")
            ?: throw IllegalArgumentException("Owner CoreCapabilitySnapshot has no scope"),
    )
    val resolutionJson = data.getJSONArray("resolutions")
    val resolutions = (0 until resolutionJson.length()).map { index ->
        val resolution = resolutionJson.getJSONObject(index)
        val rawResolution = rawJsonValueAtPath(rawDataJson, "resolutions", index.toString())
            ?: throw IllegalArgumentException("Owner CoreCapabilitySnapshot resolution is malformed")
        val key = resolution.getJSONObject("key")
        val keyScopeJson = key.getJSONObject("scope")
        val rawKeyScope = rawJsonValueAtPath(rawResolution, "key", "scope")
            ?: throw IllegalArgumentException("Owner capability resolution has no scope")
        require(parseCapabilityScope(keyScopeJson, rawKeyScope) == scope) {
            "Owner capability resolution scope does not match its snapshot"
        }
        val operation = key.getJSONObject("operation")
        CoreCapabilityResolutionObservation(
            operationCategory = operation.getString("category"),
            operation = operation.getString("operation"),
            status = parseCoreCapabilityStatus(resolution.getJSONObject("status")),
        )
    }
    require(resolutions.map { it.operationCategory to it.operation }.distinct().size == resolutions.size) {
        "Owner CoreCapabilitySnapshot has duplicate operation resolutions"
    }
    return CoreCapabilitySnapshotObservation(scope, resolutions)
}

private fun parseCapabilityScope(value: JSONObject, rawScopeJson: String): CoreCapabilityScopeObservation {
    val rawAdapterRevision = rawJsonValueAtPath(rawScopeJson, "adapter_revision")
        ?: throw IllegalArgumentException("Owner capability scope has no adapter revision")
    val host = when (val rawHost = value.getString("host")) {
        "Desktop" -> CoreHostKindObservation.Desktop
        "Android" -> CoreHostKindObservation.Android
        "Ios" -> CoreHostKindObservation.Ios
        else -> throw IllegalArgumentException("Unknown Core capability host: $rawHost")
    }
    val binaryTarget = when (val rawTarget = value.getString("binary_target")) {
        "x86_64-unknown-linux-gnu" -> CoreBinaryTargetObservation.X86_64UnknownLinuxGnu
        "aarch64-unknown-linux-gnu" -> CoreBinaryTargetObservation.Aarch64UnknownLinuxGnu
        "x86_64-unknown-linux-musl" -> CoreBinaryTargetObservation.X86_64UnknownLinuxMusl
        "aarch64-unknown-linux-musl" -> CoreBinaryTargetObservation.Aarch64UnknownLinuxMusl
        "x86_64-pc-windows-msvc" -> CoreBinaryTargetObservation.X86_64PcWindowsMsvc
        "aarch64-pc-windows-msvc" -> CoreBinaryTargetObservation.Aarch64PcWindowsMsvc
        "x86_64-pc-windows-gnu" -> CoreBinaryTargetObservation.X86_64PcWindowsGnu
        "x86_64-pc-windows-gnullvm" -> CoreBinaryTargetObservation.X86_64PcWindowsGnullvm
        "aarch64-pc-windows-gnullvm" -> CoreBinaryTargetObservation.Aarch64PcWindowsGnullvm
        "x86_64-apple-darwin" -> CoreBinaryTargetObservation.X86_64AppleDarwin
        "aarch64-apple-darwin" -> CoreBinaryTargetObservation.Aarch64AppleDarwin
        "x86_64-linux-android" -> CoreBinaryTargetObservation.X86_64LinuxAndroid
        "aarch64-linux-android" -> CoreBinaryTargetObservation.Aarch64LinuxAndroid
        "x86_64-apple-ios" -> CoreBinaryTargetObservation.X86_64AppleIos
        "aarch64-apple-ios" -> CoreBinaryTargetObservation.Aarch64AppleIos
        "aarch64-apple-ios-sim" -> CoreBinaryTargetObservation.Aarch64AppleIosSim
        "aarch64-apple-ios-macabi" -> CoreBinaryTargetObservation.Aarch64AppleIosMacabi
        "unregistered" -> CoreBinaryTargetObservation.Unregistered
        else -> throw IllegalArgumentException("Unknown Core capability binary target: $rawTarget")
    }
    require(binaryTarget.host == null || binaryTarget.host == host) {
        "Owner capability host does not match its binary target"
    }
    return CoreCapabilityScopeObservation(
        engineId = value.getString("engine_id"),
        coreVersion = value.nullableString("core_version"),
        host = host,
        binaryTarget = binaryTarget,
        adapterRevision = parseUnsignedDecimal(rawAdapterRevision, "adapter_revision"),
    )
}

private fun parseCoreCapabilityStatus(value: JSONObject): CoreCapabilityStatusObservation =
    when (val status = value.getString("status")) {
        "available" -> CoreCapabilityStatusObservation.Available(
            access = value.getString("access"),
            maturity = value.getString("maturity"),
        )
        "unsupported" -> CoreCapabilityStatusObservation.Unsupported(value.capabilityBlockerReasons())
        "version_unavailable" -> CoreCapabilityStatusObservation.VersionUnavailable(value.capabilityBlockerReasons())
        "host_unavailable" -> CoreCapabilityStatusObservation.HostUnavailable(value.capabilityBlockerReasons())
        "adapter_missing" -> CoreCapabilityStatusObservation.AdapterMissing(value.capabilityBlockerReasons())
        "unregistered" -> CoreCapabilityStatusObservation.Unregistered
        "scope_mismatch" -> CoreCapabilityStatusObservation.ScopeMismatch
        else -> throw IllegalArgumentException("Unknown CoreCapabilityStatus: $status")
    }

private fun JSONObject.capabilityBlockerReasons(): List<String> = optJSONArray("blockers")
    ?.mapObjects { it.getString("reason") }
    .orEmpty()

internal fun outboundProbeCapability(
    inventory: OutboundInventoryPageState,
    capabilities: CoreCapabilitySnapshotPageState,
): OutboundProbeOperationCapability {
    val inventoryScope = (inventory as? OutboundInventoryPageState.Observed)?.snapshot?.scope
        ?: return when (inventory) {
            is OutboundInventoryPageState.Unavailable -> OutboundProbeOperationCapability.Unavailable(inventory.failure.toLocalizedFailure())
            is OutboundInventoryPageState.Failed -> OutboundProbeOperationCapability.Unavailable(inventory.failure.toLocalizedFailure())
            else -> OutboundProbeOperationCapability.Unknown
        }
    val capabilitySnapshot = when (capabilities) {
        CoreCapabilitySnapshotPageState.Loading -> return OutboundProbeOperationCapability.Unknown
        is CoreCapabilitySnapshotPageState.Unavailable -> return OutboundProbeOperationCapability.Unavailable(capabilities.failure.toLocalizedFailure())
        is CoreCapabilitySnapshotPageState.Failed -> return OutboundProbeOperationCapability.Unavailable(capabilities.failure.toLocalizedFailure())
        is CoreCapabilitySnapshotPageState.Observed -> capabilities.snapshot
    }
    if (capabilitySnapshot.scope.engineId != inventoryScope.identity.engineId ||
        capabilitySnapshot.scope.coreVersion != inventoryScope.identity.version
    ) {
        return OutboundProbeOperationCapability.Unavailable(localizedFailure(FailureTextKey.InvalidState))
    }
    val matches = capabilitySnapshot.resolutions.filter {
        it.operationCategory == "outbound" && it.operation == "probe"
    }
    val resolution = matches.singleOrNull()
        ?: return OutboundProbeOperationCapability.Unsupported(localizedFailure(FailureTextKey.Unsupported))
    return when (val status = resolution.status) {
        is CoreCapabilityStatusObservation.Available -> when {
            status.access == "supported" -> OutboundProbeOperationCapability.Supported
            status.access == "read_only" -> OutboundProbeOperationCapability.Unsupported(
                localizedFailure(FailureTextKey.Unsupported),
            )
            else -> OutboundProbeOperationCapability.Unavailable(
                localizedFailure(FailureTextKey.NotReady, "Unknown Outbound.Probe access: ${status.access}"),
            )
        }
        is CoreCapabilityStatusObservation.Unsupported -> OutboundProbeOperationCapability.Unsupported(
            localizedFailure(FailureTextKey.Unsupported, status.reasons.firstOrNull()),
        )
        is CoreCapabilityStatusObservation.VersionUnavailable -> OutboundProbeOperationCapability.Unavailable(
            localizedFailure(FailureTextKey.NotReady, status.reasons.firstOrNull()),
        )
        is CoreCapabilityStatusObservation.HostUnavailable -> OutboundProbeOperationCapability.Unavailable(
            localizedFailure(FailureTextKey.NotReady, status.reasons.firstOrNull()),
        )
        is CoreCapabilityStatusObservation.AdapterMissing -> OutboundProbeOperationCapability.Unavailable(
            localizedFailure(FailureTextKey.NotReady, status.reasons.firstOrNull()),
        )
        CoreCapabilityStatusObservation.Unregistered -> OutboundProbeOperationCapability.Unsupported(
            localizedFailure(FailureTextKey.Unsupported),
        )
        CoreCapabilityStatusObservation.ScopeMismatch -> OutboundProbeOperationCapability.Unavailable(
            localizedFailure(FailureTextKey.InvalidState),
        )
    }
}

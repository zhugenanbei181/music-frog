package com.musicfrog.infiltrator.ui.common

import androidx.annotation.StringRes
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.musicfrog.infiltrator.R
import infiltrator_android.FfiErrorCode
import infiltrator_android.FfiFailureLocalizationKey
import infiltrator_android.FfiStatus

internal enum class FailureTextKey(@StringRes val resource: Int) {
    InvalidInput(R.string.failure_invalid_input),
    InvalidState(R.string.failure_invalid_state),
    NotReady(R.string.failure_not_ready),
    Unsupported(R.string.failure_unsupported),
    Network(R.string.failure_network),
    Authentication(R.string.failure_authentication),
    Sync(R.string.failure_sync),
    Configuration(R.string.failure_configuration),
    Storage(R.string.failure_storage),
    Permission(R.string.failure_permission),
    Canceled(R.string.failure_canceled),
    Internal(R.string.failure_internal),
    YamlSyntax(R.string.failure_yaml_syntax),
    MixinYaml(R.string.failure_mixin_yaml),
    ActiveProfileInconsistent(R.string.failure_active_profile_inconsistent),
    QuotaSourceChanged(R.string.failure_quota_source_changed),
    CurrentTimeUnavailable(R.string.failure_current_time_unavailable),
    DownloadCanceled(R.string.download_canceled),
    SyncIntervalInvalid(R.string.failure_sync_interval_invalid),
    TunMtuInvalid(R.string.failure_tun_mtu_invalid),
    TunMtuZero(R.string.failure_tun_mtu_zero),
    EmptyRule(R.string.failure_rule_empty),
}

internal enum class EmptyStateTextKey(@StringRes val resource: Int) {
    Profiles(R.string.empty_profiles),
    Outbounds(R.string.empty_outbounds),
}

internal enum class OwnerUiStatusTextKey(@StringRes val resource: Int) {
    OutboundInventoryUnknown(R.string.owner_outbound_inventory_unknown),
    OutboundInventoryUnsupported(R.string.owner_outbound_inventory_unsupported),
    OutboundDetailsLoading(R.string.owner_outbound_details_loading),
    OutboundDetailsUnknown(R.string.owner_outbound_details_unknown),
    OutboundDetailsEmpty(R.string.empty_outbound_details),
    OutboundDetailsUnsupported(R.string.owner_outbound_details_unsupported),
    OutboundDetailsNotObserved(R.string.owner_outbound_details_not_observed),
    OutboundPreferencesLoading(R.string.owner_outbound_preferences_loading),
    OutboundPreferencesUnavailable(R.string.owner_outbound_preferences_unavailable),
    ValueUnknown(R.string.owner_value_unknown),
    ValueUnsupported(R.string.owner_value_unsupported),
    LatencyHistoryEmpty(R.string.empty_latency_samples),
    EgressHistoryEmpty(R.string.empty_egress_samples),
    ProbeAvailabilityUnknown(R.string.owner_probe_availability_unknown),
    ProbeUnsupported(R.string.owner_probe_unsupported),
    ProbeCapabilityUnknown(R.string.owner_probe_capability_unknown),
    ProbeAvailable(R.string.owner_probe_available),
    ProbeTimedOut(R.string.owner_probe_timed_out),
    ProbeWorking(R.string.owner_probe_working),
    SelectionUnknown(R.string.owner_selection_unknown),
    SelectionEmpty(R.string.empty_outbound_selection),
    SelectionUnsupported(R.string.owner_selection_unsupported),
    SelectionReadOnly(R.string.owner_selection_read_only),
    SelectionAutomatic(R.string.owner_selection_automatic),
    SelectionCapabilityUnknown(R.string.owner_selection_capability_unknown),
}

internal data class LocalizedFailure(
    val key: FailureTextKey,
    val arguments: List<String> = emptyList(),
    val diagnostic: String? = null,
)

/** Application-level failure identity plus the shared localization projection. */
internal enum class AppFailureCode {
    InvalidInput,
    InvalidState,
    NotReady,
    Unsupported,
    Network,
    Authentication,
    Sync,
    Configuration,
    Storage,
    Permission,
    Canceled,
    Internal,
}

internal data class AppFailure(
    val code: AppFailureCode,
    val localization: FailureTextKey,
    val params: List<String> = emptyList(),
    val diagnostic: String? = null,
)

internal fun AppFailure.toLocalizedFailure(): LocalizedFailure = LocalizedFailure(
    key = localization,
    arguments = params,
    diagnostic = diagnostic,
)

internal fun FfiStatus.toAppFailure(): AppFailure {
    require(code != FfiErrorCode.OK) { "successful FFI status is not an application failure" }
    val projection = checkNotNull(localization) {
        "failed FFI status has no typed localization projection"
    }
    val key = projection.key.toFailureTextKey()
    val params = when (key) {
        FailureTextKey.YamlSyntax -> listOf(
            requireNotNull(projection.line).toString(),
            requireNotNull(projection.column).toString(),
        )
        else -> emptyList()
    }
    return AppFailure(
        code = code.toAppFailureCode(),
        localization = key,
        params = params,
        diagnostic = message?.takeIf(String::isNotBlank),
    )
}

private fun FfiFailureLocalizationKey.toFailureTextKey(): FailureTextKey = when (this) {
    FfiFailureLocalizationKey.INVALID_INPUT -> FailureTextKey.InvalidInput
    FfiFailureLocalizationKey.INVALID_STATE -> FailureTextKey.InvalidState
    FfiFailureLocalizationKey.NOT_READY -> FailureTextKey.NotReady
    FfiFailureLocalizationKey.UNSUPPORTED -> FailureTextKey.Unsupported
    FfiFailureLocalizationKey.NETWORK -> FailureTextKey.Network
    FfiFailureLocalizationKey.AUTHENTICATION -> FailureTextKey.Authentication
    FfiFailureLocalizationKey.SYNC -> FailureTextKey.Sync
    FfiFailureLocalizationKey.CONFIGURATION -> FailureTextKey.Configuration
    FfiFailureLocalizationKey.STORAGE -> FailureTextKey.Storage
    FfiFailureLocalizationKey.PERMISSION -> FailureTextKey.Permission
    FfiFailureLocalizationKey.CANCELED -> FailureTextKey.Canceled
    FfiFailureLocalizationKey.INTERNAL -> FailureTextKey.Internal
    FfiFailureLocalizationKey.YAML_SYNTAX -> FailureTextKey.YamlSyntax
    FfiFailureLocalizationKey.MIXIN_YAML -> FailureTextKey.MixinYaml
    FfiFailureLocalizationKey.ACTIVE_PROFILE_INCONSISTENT -> FailureTextKey.ActiveProfileInconsistent
    FfiFailureLocalizationKey.QUOTA_SOURCE_CHANGED -> FailureTextKey.QuotaSourceChanged
    FfiFailureLocalizationKey.CURRENT_TIME_UNAVAILABLE -> FailureTextKey.CurrentTimeUnavailable
    FfiFailureLocalizationKey.DOWNLOAD_CANCELED -> FailureTextKey.DownloadCanceled
}

private fun FfiErrorCode.toAppFailureCode(): AppFailureCode = when (this) {
    FfiErrorCode.OK -> error("successful FFI status is not an application failure")
    FfiErrorCode.INVALID_INPUT -> AppFailureCode.InvalidInput
    FfiErrorCode.INVALID_STATE -> AppFailureCode.InvalidState
    FfiErrorCode.NOT_READY -> AppFailureCode.NotReady
    FfiErrorCode.NOT_SUPPORTED -> AppFailureCode.Unsupported
    FfiErrorCode.IO -> AppFailureCode.Storage
    FfiErrorCode.NETWORK -> AppFailureCode.Network
    FfiErrorCode.AUTH -> AppFailureCode.Authentication
    FfiErrorCode.SYNC -> AppFailureCode.Sync
    FfiErrorCode.CONFIG -> AppFailureCode.Configuration
    FfiErrorCode.CANCELED -> AppFailureCode.Canceled
    FfiErrorCode.UNKNOWN -> AppFailureCode.Internal
}

internal fun LocalizedFailure.toAppFailure(): AppFailure = AppFailure(
    code = key.toAppFailureCode(),
    localization = key,
    params = arguments,
    diagnostic = diagnostic,
)

private fun FailureTextKey.toAppFailureCode(): AppFailureCode = when (this) {
    FailureTextKey.InvalidInput -> AppFailureCode.InvalidInput
    FailureTextKey.InvalidState -> AppFailureCode.InvalidState
    FailureTextKey.NotReady -> AppFailureCode.NotReady
    FailureTextKey.Unsupported -> AppFailureCode.Unsupported
    FailureTextKey.Network -> AppFailureCode.Network
    FailureTextKey.Authentication -> AppFailureCode.Authentication
    FailureTextKey.Sync -> AppFailureCode.Sync
    FailureTextKey.Configuration -> AppFailureCode.Configuration
    FailureTextKey.Storage -> AppFailureCode.Storage
    FailureTextKey.Permission -> AppFailureCode.Permission
    FailureTextKey.Canceled,
    FailureTextKey.DownloadCanceled -> AppFailureCode.Canceled
    FailureTextKey.Internal -> AppFailureCode.Internal
    FailureTextKey.YamlSyntax,
    FailureTextKey.MixinYaml,
    FailureTextKey.ActiveProfileInconsistent,
    FailureTextKey.QuotaSourceChanged,
    FailureTextKey.CurrentTimeUnavailable,
    FailureTextKey.SyncIntervalInvalid,
    FailureTextKey.TunMtuInvalid,
    FailureTextKey.TunMtuZero,
    FailureTextKey.EmptyRule -> AppFailureCode.InvalidState
}

internal fun FfiStatus.toLocalizedFailure(): LocalizedFailure {
    return toAppFailure().toLocalizedFailure()
}

internal fun localizedFailure(
    key: FailureTextKey,
    diagnostic: String? = null,
): LocalizedFailure = LocalizedFailure(key, diagnostic = diagnostic?.takeIf(String::isNotBlank))

internal fun OwnerFailure.toLocalizedFailure(): LocalizedFailure {
    val typedReason = when (val reason = this.reason) {
        is OwnerFailureReason.YamlSyntax -> FailureTextKey.YamlSyntax to listOf(
            reason.line.toString(),
            reason.column.toString(),
        )
        OwnerFailureReason.MixinYaml -> FailureTextKey.MixinYaml to emptyList()
        OwnerFailureReason.ActiveProfileInconsistent -> FailureTextKey.ActiveProfileInconsistent to emptyList()
        OwnerFailureReason.QuotaSourceChanged -> FailureTextKey.QuotaSourceChanged to emptyList()
        OwnerFailureReason.CurrentTimeUnavailable -> FailureTextKey.CurrentTimeUnavailable to emptyList()
        OwnerFailureReason.DownloadCanceled -> FailureTextKey.DownloadCanceled to emptyList()
        null -> null
    }
    val (key, arguments) = typedReason ?: when (code) {
        OwnerErrorCode.InvalidInput -> FailureTextKey.InvalidInput to emptyList()
        OwnerErrorCode.InvalidState -> FailureTextKey.InvalidState to emptyList()
        OwnerErrorCode.NotReady -> FailureTextKey.NotReady to emptyList()
        OwnerErrorCode.Unsupported -> FailureTextKey.Unsupported to emptyList()
        OwnerErrorCode.Network -> FailureTextKey.Network to emptyList()
        OwnerErrorCode.Authentication -> FailureTextKey.Authentication to emptyList()
        OwnerErrorCode.Configuration -> FailureTextKey.Configuration to emptyList()
        OwnerErrorCode.Storage -> FailureTextKey.Storage to emptyList()
        OwnerErrorCode.Permission -> FailureTextKey.Permission to emptyList()
        OwnerErrorCode.Canceled -> FailureTextKey.Canceled to emptyList()
        OwnerErrorCode.Internal -> FailureTextKey.Internal to emptyList()
    }
    return LocalizedFailure(key, arguments, message.takeIf(String::isNotBlank))
}

internal fun OwnerFailure.toAppFailure(): AppFailure = toLocalizedFailure().toAppFailure().copy(
    code = when (code) {
        OwnerErrorCode.InvalidInput -> AppFailureCode.InvalidInput
        OwnerErrorCode.InvalidState -> AppFailureCode.InvalidState
        OwnerErrorCode.NotReady -> AppFailureCode.NotReady
        OwnerErrorCode.Unsupported -> AppFailureCode.Unsupported
        OwnerErrorCode.Network -> AppFailureCode.Network
        OwnerErrorCode.Authentication -> AppFailureCode.Authentication
        OwnerErrorCode.Configuration -> AppFailureCode.Configuration
        OwnerErrorCode.Storage -> AppFailureCode.Storage
        OwnerErrorCode.Permission -> AppFailureCode.Permission
        OwnerErrorCode.Canceled -> AppFailureCode.Canceled
        OwnerErrorCode.Internal -> AppFailureCode.Internal
    },
)

@Composable
internal fun LocalizedFailureText(
    failure: LocalizedFailure,
    modifier: Modifier = Modifier,
) {
    Column(modifier = modifier) {
        Text(
            text = stringResource(failure.key.resource, *failure.arguments.toTypedArray()),
            style = MaterialTheme.typography.bodyMedium,
        )
        failure.diagnostic?.let { detail ->
            Text(
                text = stringResource(R.string.failure_diagnostic_detail, detail),
                modifier = Modifier.padding(top = 4.dp),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}

@Composable
internal fun LocalizedFailure.asDisplayString(): String {
    val title = stringResource(key.resource, *arguments.toTypedArray())
    val detail = diagnostic?.let { stringResource(R.string.failure_diagnostic_detail, it) }
    return listOfNotNull(title, detail).joinToString("\n")
}

@Composable
internal fun EmptyStateText(
    key: EmptyStateTextKey,
    modifier: Modifier = Modifier,
) {
    Text(
        text = stringResource(key.resource),
        modifier = modifier,
        style = MaterialTheme.typography.bodyMedium,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
    )
}

@Composable
internal fun OwnerUiStatusText(
    key: OwnerUiStatusTextKey,
    modifier: Modifier = Modifier,
) {
    Text(
        text = stringResource(key.resource),
        modifier = modifier,
        style = MaterialTheme.typography.bodySmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
    )
}

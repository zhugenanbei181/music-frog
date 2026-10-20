package com.musicfrog.infiltrator.ui.settings.dns

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.musicfrog.infiltrator.ui.common.AppFailure
import com.musicfrog.infiltrator.ui.common.DEFAULT_FFI_TIMEOUT_MS
import com.musicfrog.infiltrator.ui.common.FailureTextKey
import com.musicfrog.infiltrator.ui.common.LONG_FFI_TIMEOUT_MS
import com.musicfrog.infiltrator.ui.common.localizedFailure
import com.musicfrog.infiltrator.ui.common.toAppFailure
import infiltrator_android.AndroidOwnerOperation
import infiltrator_android.DnsFallbackFilterSettings
import infiltrator_android.DnsSettings
import infiltrator_android.DnsSettingsPatch
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

internal data class DnsUiState(
    val enabled: DnsToggleState = DnsToggleState.Unspecified,
    val ipv6: DnsToggleState = DnsToggleState.Unspecified,
    val enhancedMode: DnsEnhancedMode = DnsEnhancedMode.Unspecified,
    val nameserver: String = "",
    val defaultNameserver: String = "",
    val fallback: String = "",
    val fallbackFilterGeoip: DnsGeoIpPolicy = DnsGeoIpPolicy.Automatic,
    val fallbackFilterGeoipCode: String = "",
    val fallbackFilterIpcidr: String = "",
    val fallbackFilterDomain: String = "",
    val fallbackFilterDomainSuffix: String = "",
    val observation: DnsSettingsReadState = DnsSettingsReadState.Loading(),
    val operation: DnsSettingsOperationState = DnsSettingsOperationState.Idle,
) {
    val availability: DnsSettingsAvailability
        get() = when (lastAcceptedDns(observation)) {
            null -> DnsSettingsAvailability.Unavailable
            else -> DnsSettingsAvailability.Available
        }

    val activity: DnsActivity
        get() = when (operation) {
            DnsSettingsOperationState.Reading,
            DnsSettingsOperationState.Saving,
            -> DnsActivity.Busy

            DnsSettingsOperationState.Idle,
            is DnsSettingsOperationState.Saved,
            is DnsSettingsOperationState.Failed,
            is DnsSettingsOperationState.Cancelled,
            is DnsSettingsOperationState.TimedOut,
            is DnsSettingsOperationState.Unconfirmed,
            -> DnsActivity.Interactive
        }
}

internal class DnsViewModel(private val repository: DnsRepository) : ViewModel() {
    constructor() : this(UniFfiDnsRepository())

    private val _state = MutableStateFlow(DnsUiState())
    val state: StateFlow<DnsUiState> = _state.asStateFlow()

    private var requestSequence = 0uL
    private var latestRequestId = DnsRequestId(0uL)

    init {
        load()
    }

    fun load() {
        when (_state.value.activity) {
            DnsActivity.Busy -> return
            DnsActivity.Interactive -> Unit
        }
        val request = nextRequestId()
        val accepted = lastAcceptedDns(_state.value.observation)
        _state.value = _state.value.copy(
            observation = DnsSettingsReadState.Loading(accepted),
            operation = DnsSettingsOperationState.Reading,
        )
        viewModelScope.launch {
            handleReadResult(
                repository.execute(AndroidOwnerOperation.DnsSettings, DEFAULT_FFI_TIMEOUT_MS),
                request,
            )
        }
    }

    fun updateEnabled(value: DnsToggleState) = updateDraft { copy(enabled = value) }
    fun updateIpv6(value: DnsToggleState) = updateDraft { copy(ipv6 = value) }
    fun updateEnhancedMode(value: DnsEnhancedMode) = updateDraft { copy(enhancedMode = value) }
    fun updateNameserver(value: String) = updateDraft { copy(nameserver = value) }
    fun updateDefaultNameserver(value: String) = updateDraft { copy(defaultNameserver = value) }
    fun updateFallback(value: String) = updateDraft { copy(fallback = value) }
    fun updateFallbackFilterGeoip(value: DnsGeoIpPolicy) = updateDraft { copy(fallbackFilterGeoip = value) }
    fun updateFallbackFilterGeoipCode(value: String) = updateDraft { copy(fallbackFilterGeoipCode = value) }
    fun updateFallbackFilterIpcidr(value: String) = updateDraft { copy(fallbackFilterIpcidr = value) }
    fun updateFallbackFilterDomain(value: String) = updateDraft { copy(fallbackFilterDomain = value) }
    fun updateFallbackFilterDomainSuffix(value: String) = updateDraft { copy(fallbackFilterDomainSuffix = value) }

    fun save() {
        when (_state.value.activity) {
            DnsActivity.Busy -> return
            DnsActivity.Interactive -> Unit
        }
        if (lastAcceptedDns(_state.value.observation) == null) {
            setUnconfirmed(notReadyFailure())
            return
        }
        val current = _state.value
        val intent = current.toSaveIntent()
        val request = nextRequestId()
        _state.value = current.copy(operation = DnsSettingsOperationState.Saving)
        viewModelScope.launch {
            handleWriteResult(
                repository.execute(
                    AndroidOwnerOperation.DnsSettingsSave(intent.patch),
                    LONG_FFI_TIMEOUT_MS,
                ),
                request,
                intent,
            )
        }
    }

    fun clearError() {
        when (_state.value.operation) {
            is DnsSettingsOperationState.Failed,
            is DnsSettingsOperationState.Cancelled,
            is DnsSettingsOperationState.TimedOut,
            is DnsSettingsOperationState.Unconfirmed,
            -> {
                _state.value = _state.value.copy(operation = DnsSettingsOperationState.Idle)
            }

            DnsSettingsOperationState.Idle,
            DnsSettingsOperationState.Reading,
            DnsSettingsOperationState.Saving,
            is DnsSettingsOperationState.Saved,
            -> Unit
        }
    }

    private fun updateDraft(update: DnsUiState.() -> DnsUiState) {
        val current = _state.value
        when (current.activity) {
            DnsActivity.Busy -> return
            DnsActivity.Interactive -> Unit
        }
        when (lastAcceptedDns(current.observation)) {
            null -> return
            else -> _state.value = current.update().copy(operation = DnsSettingsOperationState.Idle)
        }
    }

    private fun handleReadResult(result: DnsRepositoryResult, request: DnsRequestId): DnsRequestOutcome {
        when (requestAdmission(request)) {
            DnsRequestAdmission.Superseded -> return DnsRequestOutcome.Superseded
            DnsRequestAdmission.Current -> Unit
        }
        return when (result) {
            is DnsRepositoryResult.Completed -> when (accept(result.point, DnsSettingsOperationState.Idle)) {
                DnsSnapshotAdmission.Accepted -> DnsRequestOutcome.Accepted

                DnsSnapshotAdmission.Stale -> {
                    failRead(notReadyFailure())
                    DnsRequestOutcome.Rejected
                }
            }

            is DnsRepositoryResult.Failed -> {
                acceptLatest(result.latest)
                failRead(result.failure)
                DnsRequestOutcome.Rejected
            }

            is DnsRepositoryResult.Cancelled -> {
                acceptLatest(result.latest)
                markUnavailable(result.failure)
                _state.value = _state.value.copy(operation = DnsSettingsOperationState.Cancelled(result.failure))
                DnsRequestOutcome.Rejected
            }

            is DnsRepositoryResult.TimedOut -> {
                markUnavailable(result.failure)
                _state.value = _state.value.copy(operation = DnsSettingsOperationState.TimedOut(result.failure))
                DnsRequestOutcome.Rejected
            }

            is DnsRepositoryResult.Unconfirmed -> {
                acceptLatest(result.latest)
                markUnavailable(result.failure)
                _state.value = _state.value.copy(operation = DnsSettingsOperationState.Unconfirmed(result.failure))
                DnsRequestOutcome.Rejected
            }
        }
    }

    private fun handleWriteResult(result: DnsRepositoryResult, request: DnsRequestId, intent: DnsSaveIntent): DnsRequestOutcome {
        when (requestAdmission(request)) {
            DnsRequestAdmission.Superseded -> return DnsRequestOutcome.Superseded
            DnsRequestAdmission.Current -> Unit
        }
        return when (result) {
            is DnsRepositoryResult.Completed -> when {
                result.point.value.matches(
                    patch = intent.patch,
                    expectedEnhancedMode = intent.expectedEnhancedMode,
                    expectedFallbackFilter = intent.expectedFallbackFilter,
                ) != DnsSettingsReadbackResult.Confirmed -> {
                    setUnconfirmed(notReadyFailure())
                    DnsRequestOutcome.Rejected
                }

                else -> when (
                    accept(
                        result.point,
                        DnsSettingsOperationState.Saved(result.point.revision),
                    )
                ) {
                    DnsSnapshotAdmission.Accepted -> DnsRequestOutcome.Accepted

                    DnsSnapshotAdmission.Stale -> {
                        setUnconfirmed(notReadyFailure())
                        DnsRequestOutcome.Rejected
                    }
                }
            }

            is DnsRepositoryResult.Failed -> {
                acceptLatest(result.latest)
                _state.value = _state.value.copy(operation = DnsSettingsOperationState.Failed(result.failure))
                DnsRequestOutcome.Rejected
            }

            is DnsRepositoryResult.Cancelled -> {
                acceptLatest(result.latest)
                _state.value = _state.value.copy(operation = DnsSettingsOperationState.Cancelled(result.failure))
                DnsRequestOutcome.Rejected
            }

            is DnsRepositoryResult.TimedOut -> {
                _state.value = _state.value.copy(operation = DnsSettingsOperationState.TimedOut(result.failure))
                DnsRequestOutcome.Rejected
            }

            is DnsRepositoryResult.Unconfirmed -> {
                acceptLatest(result.latest)
                setUnconfirmed(result.failure)
                DnsRequestOutcome.Rejected
            }
        }
    }

    private fun accept(point: DnsSettingsObservationPoint, operation: DnsSettingsOperationState): DnsSnapshotAdmission =
        when (acceptsDnsRevision(_state.value.observation, point.revision)) {
            DnsSnapshotAdmission.Accepted -> {
                acceptSettings(point, operation)
                DnsSnapshotAdmission.Accepted
            }

            DnsSnapshotAdmission.Stale -> DnsSnapshotAdmission.Stale
        }

    private fun acceptLatest(point: DnsSettingsObservationPoint?) {
        when (point) {
            null -> Unit
            else -> accept(point, _state.value.operation)
        }
    }

    private fun acceptSettings(point: DnsSettingsObservationPoint, operation: DnsSettingsOperationState) {
        val settings = point.value
        val fallbackFilter = settings.fallbackFilter
        _state.value = DnsUiState(
            enabled = settings.enable.toDnsToggleState(),
            ipv6 = settings.ipv6.toDnsToggleState(),
            enhancedMode = dnsEnhancedModeFromReadback(settings.enhancedMode),
            nameserver = settings.nameserver.joinToString(separator = "\n"),
            defaultNameserver = settings.defaultNameserver.joinToString(separator = "\n"),
            fallback = settings.fallback.joinToString(separator = "\n"),
            fallbackFilterGeoip = when (fallbackFilter?.geoip) {
                true -> DnsGeoIpPolicy.Enabled
                false -> DnsGeoIpPolicy.Disabled
                null -> DnsGeoIpPolicy.Automatic
            },
            fallbackFilterGeoipCode = fallbackFilter?.geoipCode ?: "",
            fallbackFilterIpcidr = fallbackFilter?.ipcidr?.joinToString(separator = "\n") ?: "",
            fallbackFilterDomain = fallbackFilter?.domain?.joinToString(separator = "\n") ?: "",
            fallbackFilterDomainSuffix = fallbackFilter?.domainSuffix?.joinToString(separator = "\n") ?: "",
            observation = DnsSettingsReadState.Observed(settings, point.revision),
            operation = operation,
        )
    }

    private fun failRead(failure: AppFailure) {
        markUnavailable(failure)
        _state.value = _state.value.copy(operation = DnsSettingsOperationState.Failed(failure))
    }

    private fun markUnavailable(failure: AppFailure) {
        _state.value = _state.value.copy(
            observation = unavailableDns(_state.value.observation, failure),
        )
    }

    private fun setUnconfirmed(failure: AppFailure) {
        markUnavailable(failure)
        _state.value = _state.value.copy(operation = DnsSettingsOperationState.Unconfirmed(failure))
    }

    private data class DnsSaveIntent(
        val patch: DnsSettingsPatch,
        val expectedEnhancedMode: String?,
        val expectedFallbackFilter: DnsFallbackFilterSettings?,
    )

    private fun DnsUiState.toSaveIntent(): DnsSaveIntent {
        val previousMode = lastAcceptedDns(observation)?.value?.enhancedMode
        val enhancedModeValue = enhancedMode.toPatchValue()
        val fallbackFilter = buildFallbackFilter(this)
        return DnsSaveIntent(
            patch = DnsSettingsPatch(
                enabled.toPatchValue(),
                ipv6.toPatchValue(),
                enhancedModeValue,
                parseList(nameserver),
                parseList(defaultNameserver),
                parseList(fallback),
                fallbackFilter,
            ),
            expectedEnhancedMode = enhancedMode.expectedReadback(previousMode),
            expectedFallbackFilter = fallbackFilter,
        )
    }

    private fun buildFallbackFilter(state: DnsUiState): DnsFallbackFilterSettings? {
        val geoipCode = state.fallbackFilterGeoipCode.trim().ifBlank { null }
        val ipcidr = parseList(state.fallbackFilterIpcidr)
        val domain = parseList(state.fallbackFilterDomain)
        val domainSuffix = parseList(state.fallbackFilterDomainSuffix)
        val geoip = when (state.fallbackFilterGeoip) {
            DnsGeoIpPolicy.Automatic -> null
            DnsGeoIpPolicy.Enabled -> true
            DnsGeoIpPolicy.Disabled -> false
        }
        if (geoip == null && geoipCode == null && ipcidr.isEmpty() && domain.isEmpty() && domainSuffix.isEmpty()) {
            return null
        }
        return DnsFallbackFilterSettings(geoip, geoipCode, ipcidr, domain, domainSuffix)
    }

    private fun parseList(raw: String): List<String> = raw.split('\n', ',')
        .map { it.trim() }
        .filter { it.isNotEmpty() }

    @Synchronized
    private fun nextRequestId(): DnsRequestId {
        check(requestSequence < ULong.MAX_VALUE) { "DNS request sequence is exhausted" }
        requestSequence += 1u
        latestRequestId = DnsRequestId(requestSequence)
        return latestRequestId
    }

    private fun requestAdmission(request: DnsRequestId): DnsRequestAdmission = when (request) {
        latestRequestId -> DnsRequestAdmission.Current
        else -> DnsRequestAdmission.Superseded
    }

    private fun notReadyFailure(): AppFailure = localizedFailure(FailureTextKey.NotReady).toAppFailure()
}

private data class DnsRequestId(val value: ULong)

private enum class DnsRequestAdmission {
    Current,
    Superseded,
}

private enum class DnsRequestOutcome {
    Accepted,
    Rejected,
    Superseded,
}

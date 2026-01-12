package com.musicfrog.despicableinfiltrator.ui.settings.rules

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.musicfrog.despicableinfiltrator.ui.common.DEFAULT_FFI_TIMEOUT_MS
import com.musicfrog.despicableinfiltrator.ui.common.LONG_FFI_TIMEOUT_MS
import com.musicfrog.despicableinfiltrator.ui.common.runFfiCall
import com.musicfrog.despicableinfiltrator.ui.common.userMessage
import infiltrator_android.FfiErrorCode
import infiltrator_android.RuleEntryRecord
import infiltrator_android.ruleProviders
import infiltrator_android.ruleProvidersSave
import infiltrator_android.rulesList
import infiltrator_android.rulesSave
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

data class RulesUiState(
    val rules: List<RuleEntryRecord> = emptyList(),
    val providersJson: String = "",
    val newRule: String = "",
    val isLoading: Boolean = false,
    val error: String? = null,
    val rulesSaved: Boolean = false,
    val providersSaved: Boolean = false
)

class RulesViewModel : ViewModel() {
    private val _state = MutableStateFlow(RulesUiState(isLoading = true))
    val state: StateFlow<RulesUiState> = _state.asStateFlow()

    init {
        load()
    }

    fun load() {
        viewModelScope.launch {
            _state.value = _state.value.copy(
                isLoading = true,
                error = null,
                rulesSaved = false,
                providersSaved = false
            )

            val rulesCall = runFfiCall(timeoutMs = DEFAULT_FFI_TIMEOUT_MS) { rulesList() }
            if (rulesCall.error != null) {
                _state.value = _state.value.copy(isLoading = false, error = rulesCall.error)
                return@launch
            }
            val rulesResult = rulesCall.value!!
            if (rulesResult.status.code == FfiErrorCode.OK) {
                _state.value = _state.value.copy(rules = rulesResult.rules)
            } else {
                _state.value = _state.value.copy(
                    error = rulesResult.status.userMessage("Failed to load rules")
                )
            }

            val providersCall = runFfiCall(timeoutMs = DEFAULT_FFI_TIMEOUT_MS) { ruleProviders() }
            if (providersCall.error != null) {
                _state.value = _state.value.copy(isLoading = false, error = providersCall.error)
                return@launch
            }
            val providersResult = providersCall.value!!
            if (providersResult.status.code == FfiErrorCode.OK) {
                _state.value = _state.value.copy(providersJson = providersResult.json)
            } else {
                _state.value = _state.value.copy(
                    error = providersResult.status.userMessage("Failed to load rule providers")
                )
            }

            _state.value = _state.value.copy(isLoading = false)
        }
    }

    fun updateNewRule(value: String) {
        _state.value = _state.value.copy(newRule = value, rulesSaved = false)
    }

    fun addRule() {
        val rule = _state.value.newRule.trim()
        if (rule.isEmpty()) {
            _state.value = _state.value.copy(error = "Rule cannot be empty")
            return
        }
        val updated = _state.value.rules + RuleEntryRecord(rule, true)
        _state.value = _state.value.copy(rules = updated, newRule = "", rulesSaved = false)
    }

    fun toggleRule(index: Int, enabled: Boolean) {
        val current = _state.value.rules.toMutableList()
        if (index < 0 || index >= current.size) {
            return
        }
        val entry = current[index]
        current[index] = RuleEntryRecord(entry.rule, enabled)
        _state.value = _state.value.copy(rules = current, rulesSaved = false)
    }

    fun removeRule(index: Int) {
        val current = _state.value.rules.toMutableList()
        if (index < 0 || index >= current.size) {
            return
        }
        current.removeAt(index)
        _state.value = _state.value.copy(rules = current, rulesSaved = false)
    }

    fun updateProvidersJson(value: String) {
        _state.value = _state.value.copy(providersJson = value, providersSaved = false)
    }

    fun saveRules() {
        viewModelScope.launch {
            val current = _state.value
            _state.value = current.copy(isLoading = true, error = null)

            val call = runFfiCall(timeoutMs = LONG_FFI_TIMEOUT_MS) {
                rulesSave(current.rules)
            }
            if (call.error != null) {
                _state.value = _state.value.copy(isLoading = false, error = call.error)
                return@launch
            }
            val result = call.value!!
            if (result.status.code == FfiErrorCode.OK) {
                _state.value = _state.value.copy(
                    rules = result.rules,
                    isLoading = false,
                    rulesSaved = true
                )
            } else {
                _state.value = _state.value.copy(
                    isLoading = false,
                    error = result.status.userMessage("Failed to save rules")
                )
            }
        }
    }

    fun saveProviders() {
        viewModelScope.launch {
            val current = _state.value
            _state.value = current.copy(isLoading = true, error = null)

            val call = runFfiCall(timeoutMs = LONG_FFI_TIMEOUT_MS) {
                ruleProvidersSave(current.providersJson)
            }
            if (call.error != null) {
                _state.value = _state.value.copy(isLoading = false, error = call.error)
                return@launch
            }
            val result = call.value!!
            if (result.status.code == FfiErrorCode.OK) {
                _state.value = _state.value.copy(
                    providersJson = result.json,
                    isLoading = false,
                    providersSaved = true
                )
            } else {
                _state.value = _state.value.copy(
                    isLoading = false,
                    error = result.status.userMessage("Failed to save rule providers")
                )
            }
        }
    }

    fun clearError() {
        _state.value = _state.value.copy(error = null)
    }
}

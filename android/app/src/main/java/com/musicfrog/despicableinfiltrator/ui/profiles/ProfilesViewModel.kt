package com.musicfrog.despicableinfiltrator.ui.profiles

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import infiltrator_android.FfiErrorCode
import infiltrator_android.ProfileSummary
import infiltrator_android.profileCreate
import infiltrator_android.profileSelect
import infiltrator_android.profileUpdate
import infiltrator_android.profilesList
import com.musicfrog.despicableinfiltrator.ui.common.DEFAULT_FFI_TIMEOUT_MS
import com.musicfrog.despicableinfiltrator.ui.common.LONG_FFI_TIMEOUT_MS
import com.musicfrog.despicableinfiltrator.ui.common.emptyMessage
import com.musicfrog.despicableinfiltrator.ui.common.runFfiCall
import com.musicfrog.despicableinfiltrator.ui.common.userMessage
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

class ProfilesViewModel : ViewModel() {
    private val _profiles = MutableStateFlow<List<ProfileSummary>>(emptyList())
    val profiles: StateFlow<List<ProfileSummary>> = _profiles.asStateFlow()

    private val _isLoading = MutableStateFlow(false)
    val isLoading: StateFlow<Boolean> = _isLoading.asStateFlow()

    private val _error = MutableStateFlow<String?>(null)
    val error: StateFlow<String?> = _error.asStateFlow()

    private val _emptyMessage = MutableStateFlow<String?>(null)
    val emptyMessage: StateFlow<String?> = _emptyMessage.asStateFlow()

    init {
        loadProfiles()
    }

    fun loadProfiles() {
        viewModelScope.launch {
            _isLoading.value = true
            _error.value = null
            _emptyMessage.value = null
            try {
                val call = runFfiCall(timeoutMs = DEFAULT_FFI_TIMEOUT_MS) { profilesList() }
                if (call.error != null) {
                    _error.value = call.error
                } else {
                    val result = call.value!!
                    if (result.status.code == FfiErrorCode.OK) {
                        _profiles.value = result.profiles
                        if (result.profiles.isEmpty()) {
                            _emptyMessage.value = emptyMessage("profiles")
                        }
                    } else {
                        _error.value = result.status.userMessage("Failed to load profiles")
                    }
                }
            } catch (e: Exception) {
                _error.value = e.message
            } finally {
                _isLoading.value = false
            }
        }
    }

    fun addProfile(name: String, url: String) {
        viewModelScope.launch {
            _isLoading.value = true
            try {
                val call = runFfiCall(timeoutMs = LONG_FFI_TIMEOUT_MS) {
                    profileCreate(name, url)
                }
                if (call.error != null) {
                    _error.value = call.error
                } else {
                    val result = call.value!!
                    if (result.code == FfiErrorCode.OK) {
                        loadProfiles()
                    } else {
                        _error.value = result.userMessage("Failed to add profile")
                    }
                }
            } catch (e: Exception) {
                _error.value = e.message
            } finally {
                _isLoading.value = false
            }
        }
    }

    fun selectProfile(name: String) {
        viewModelScope.launch {
            _isLoading.value = true
            try {
                val call = runFfiCall(timeoutMs = DEFAULT_FFI_TIMEOUT_MS) {
                    profileSelect(name)
                }
                if (call.error != null) {
                    _error.value = call.error
                } else {
                    val result = call.value!!
                    if (result.code == FfiErrorCode.OK) {
                        loadProfiles()
                    } else {
                        _error.value = result.userMessage("Failed to select profile")
                    }
                }
            } catch (e: Exception) {
                _error.value = e.message
            } finally {
                _isLoading.value = false
            }
        }
    }

    fun updateProfile(name: String) {
        viewModelScope.launch {
            _isLoading.value = true
            try {
                val call = runFfiCall(timeoutMs = LONG_FFI_TIMEOUT_MS) {
                    profileUpdate(name)
                }
                if (call.error != null) {
                    _error.value = call.error
                } else {
                    val result = call.value!!
                    if (result.code == FfiErrorCode.OK) {
                        loadProfiles()
                    } else {
                        _error.value = result.userMessage("Failed to update profile")
                    }
                }
            } catch (e: Exception) {
                _error.value = e.message
            } finally {
                _isLoading.value = false
            }
        }
    }

    fun clearError() {
        _error.value = null
    }
}

package com.musicfrog.infiltrator.ui.profiles

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import infiltrator_android.FfiErrorCode
import infiltrator_android.ProfileDetail
import infiltrator_android.ProfileSummary
import infiltrator_android.profileCreate
import infiltrator_android.profileDelete
import infiltrator_android.profileDetail
import infiltrator_android.profileSave
import infiltrator_android.profileSelect
import infiltrator_android.profileSubscriptionClear
import infiltrator_android.profileSubscriptionSave
import infiltrator_android.profileUpdate
import infiltrator_android.profilesList
import com.musicfrog.infiltrator.ui.common.DEFAULT_FFI_TIMEOUT_MS
import com.musicfrog.infiltrator.ui.common.LONG_FFI_TIMEOUT_MS
import com.musicfrog.infiltrator.ui.common.emptyMessage
import com.musicfrog.infiltrator.ui.common.runFfiCall
import com.musicfrog.infiltrator.ui.common.userMessage
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

class ProfilesViewModel : ViewModel() {
    private val _profiles = MutableStateFlow<List<ProfileSummary>>(emptyList())
    val profiles: StateFlow<List<ProfileSummary>> = _profiles.asStateFlow()

    private val _profileDetail = MutableStateFlow<ProfileDetail?>(null)
    val profileDetail: StateFlow<ProfileDetail?> = _profileDetail.asStateFlow()

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
            try {
                reloadProfiles()
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
            _error.value = null
            try {
                val call = runFfiCall(timeoutMs = LONG_FFI_TIMEOUT_MS) {
                    profileCreate(name, url)
                }
                if (call.error != null) {
                    _error.value = call.error
                } else {
                    val result = call.value!!
                    if (result.code == FfiErrorCode.OK) {
                        reloadProfiles()
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
            _error.value = null
            try {
                val call = runFfiCall(timeoutMs = DEFAULT_FFI_TIMEOUT_MS) {
                    profileSelect(name)
                }
                if (call.error != null) {
                    _error.value = call.error
                } else {
                    val result = call.value!!
                    if (result.code == FfiErrorCode.OK) {
                        reloadProfiles()
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
            _error.value = null
            try {
                val call = runFfiCall(timeoutMs = LONG_FFI_TIMEOUT_MS) {
                    profileUpdate(name)
                }
                if (call.error != null) {
                    _error.value = call.error
                } else {
                    val result = call.value!!
                    if (result.code == FfiErrorCode.OK) {
                        reloadProfiles()
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

    fun loadProfileDetail(name: String) {
        viewModelScope.launch {
            _isLoading.value = true
            _error.value = null
            try {
                val call = runFfiCall(timeoutMs = DEFAULT_FFI_TIMEOUT_MS) {
                    profileDetail(name)
                }
                if (call.error != null) {
                    _error.value = call.error
                } else {
                    val result = call.value!!
                    if (result.status.code == FfiErrorCode.OK && result.profile != null) {
                        _profileDetail.value = result.profile
                    } else {
                        _error.value = result.status.userMessage("Failed to load profile detail")
                    }
                }
            } catch (e: Exception) {
                _error.value = e.message
            } finally {
                _isLoading.value = false
            }
        }
    }

    fun saveProfileContent(name: String, content: String, activate: Boolean) {
        viewModelScope.launch {
            _isLoading.value = true
            _error.value = null
            try {
                val call = runFfiCall(timeoutMs = LONG_FFI_TIMEOUT_MS) {
                    profileSave(name, content, activate)
                }
                if (call.error != null) {
                    _error.value = call.error
                } else {
                    val result = call.value!!
                    if (result.code == FfiErrorCode.OK) {
                        _profileDetail.value = null
                        reloadProfiles()
                    } else {
                        _error.value = result.userMessage("Failed to save profile")
                    }
                }
            } catch (e: Exception) {
                _error.value = e.message
            } finally {
                _isLoading.value = false
            }
        }
    }

    fun deleteProfile(name: String) {
        viewModelScope.launch {
            _isLoading.value = true
            _error.value = null
            try {
                val call = runFfiCall(timeoutMs = DEFAULT_FFI_TIMEOUT_MS) {
                    profileDelete(name)
                }
                if (call.error != null) {
                    _error.value = call.error
                } else {
                    val result = call.value!!
                    if (result.code == FfiErrorCode.OK) {
                        reloadProfiles()
                    } else {
                        _error.value = result.userMessage("Failed to delete profile")
                    }
                }
            } catch (e: Exception) {
                _error.value = e.message
            } finally {
                _isLoading.value = false
            }
        }
    }

    fun saveSubscriptionSettings(
        name: String,
        url: String,
        autoUpdateEnabled: Boolean,
        updateIntervalHours: Int?
    ) {
        viewModelScope.launch {
            _isLoading.value = true
            _error.value = null
            try {
                val call = runFfiCall(timeoutMs = LONG_FFI_TIMEOUT_MS) {
                    profileSubscriptionSave(
                        name = name,
                        url = url,
                        autoUpdateEnabled = autoUpdateEnabled,
                        updateIntervalHours = updateIntervalHours?.toUInt()
                    )
                }
                if (call.error != null) {
                    _error.value = call.error
                } else {
                    val result = call.value!!
                    if (result.code == FfiErrorCode.OK) {
                        reloadProfiles()
                    } else {
                        _error.value = result.userMessage("Failed to save subscription settings")
                    }
                }
            } catch (e: Exception) {
                _error.value = e.message
            } finally {
                _isLoading.value = false
            }
        }
    }

    fun clearSubscriptionSettings(name: String) {
        viewModelScope.launch {
            _isLoading.value = true
            _error.value = null
            try {
                val call = runFfiCall(timeoutMs = DEFAULT_FFI_TIMEOUT_MS) {
                    profileSubscriptionClear(name)
                }
                if (call.error != null) {
                    _error.value = call.error
                } else {
                    val result = call.value!!
                    if (result.code == FfiErrorCode.OK) {
                        reloadProfiles()
                    } else {
                        _error.value = result.userMessage("Failed to clear subscription settings")
                    }
                }
            } catch (e: Exception) {
                _error.value = e.message
            } finally {
                _isLoading.value = false
            }
        }
    }

    fun clearProfileDetail() {
        _profileDetail.value = null
    }

    fun clearError() {
        _error.value = null
    }

    private suspend fun reloadProfiles() {
        _emptyMessage.value = null
        val call = runFfiCall(timeoutMs = DEFAULT_FFI_TIMEOUT_MS) { profilesList() }
        if (call.error != null) {
            _error.value = call.error
            return
        }
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
}

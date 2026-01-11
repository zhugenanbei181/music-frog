package com.musicfrog.despicableinfiltrator.ui.profiles

import android.util.Log
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import infiltrator_android.FfiErrorCode
import infiltrator_android.ProfileSummary
import infiltrator_android.profileCreate
import infiltrator_android.profileSelect
import infiltrator_android.profileUpdate
import infiltrator_android.profilesList
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

    init {
        loadProfiles()
    }

    fun loadProfiles() {
        viewModelScope.launch {
            _isLoading.value = true
            try {
                val result = profilesList()
                if (result.status.code == FfiErrorCode.OK) {
                    _profiles.value = result.profiles
                } else {
                    _error.value = result.status.message ?: "Unknown error"
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
                val result = profileCreate(name, url)
                if (result.code == FfiErrorCode.OK) {
                    loadProfiles() // Refresh list
                } else {
                    _error.value = result.message
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
                val result = profileSelect(name)
                if (result.code == FfiErrorCode.OK) {
                    loadProfiles()
                } else {
                    _error.value = result.message
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
                val result = profileUpdate(name)
                if (result.code == FfiErrorCode.OK) {
                    loadProfiles()
                } else {
                    _error.value = result.message
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

package com.musicfrog.infiltrator.ui.about

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import com.musicfrog.infiltrator.BuildConfig
import com.musicfrog.infiltrator.R

@Composable
fun AboutScreen() {
    Box(modifier = Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
        Text("${stringResource(R.string.app_name)} v${BuildConfig.VERSION_NAME}")
    }
}

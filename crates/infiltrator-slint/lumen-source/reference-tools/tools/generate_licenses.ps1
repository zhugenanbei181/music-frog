#!/usr/bin/env pwsh
# Regenerate the offline third-party license catalog from the locked Cargo dependency sources.

$ErrorActionPreference = "Stop"
$root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$output = Join-Path $root "assets\third_party_licenses.json"
$targets = @(
    "x86_64-pc-windows-msvc",
    "aarch64-linux-android",
    "x86_64-unknown-linux-gnu"
)
$maxLicenseFileBytes = 1MB

$filenameIds = @{
    "0bsd" = "0BSD"
    "apache" = "Apache-2.0"
    "apache-2.0" = "Apache-2.0"
    "apache-2.0-with-llvm-exception" = "Apache-2.0 WITH LLVM-exception"
    "bsd-2-clause" = "BSD-2-Clause"
    "bsd-3-clause" = "BSD-3-Clause"
    "boost" = "BSL-1.0"
    "bsl-1.0" = "BSL-1.0"
    "cc0" = "CC0-1.0"
    "cc0-1.0" = "CC0-1.0"
    "cdla-permissive-2.0" = "CDLA-Permissive-2.0"
    "gpl-3.0-only" = "GPL-3.0-only"
    "gpl-3.0-or-later" = "GPL-3.0-or-later"
    "gpl-2.0-only" = "GPL-2.0-only"
    "gplv2" = "GPL-2.0-only"
    "isc" = "ISC"
    "mit" = "MIT"
    "mpl-2.0" = "MPL-2.0"
    "unicode" = "Unicode-3.0"
    "unicode-3.0" = "Unicode-3.0"
    "unlicense" = "Unlicense"
    "wtfpl" = "WTFPL"
    "zlib" = "Zlib"
}

function Get-SpdxIds([string] $expression) {
    $seen = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::Ordinal)
    $ids = [System.Collections.Generic.List[string]]::new()
    $normalized = $expression -replace "/", " OR "
    foreach ($token in ($normalized -split "[\s()]+")) {
        if ($token -and $token -notin @("AND", "OR", "WITH") -and $seen.Add($token)) {
            $ids.Add($token)
        }
    }
    return $ids.ToArray()
}

function Get-LicenseFiles($package) {
    $packageRoot = Split-Path -Parent $package.manifest_path
    $paths = [System.Collections.Generic.List[string]]::new()
    foreach ($path in [System.IO.Directory]::EnumerateFiles($packageRoot)) {
        $name = [System.IO.Path]::GetFileName($path)
        if ($name -match "^(?i:LICENSE|LICENCE|COPYING)([-._]|$)") {
            $paths.Add($path)
        }
    }
    $licenseDirectory = Join-Path $packageRoot "LICENSES"
    if (Test-Path -LiteralPath $licenseDirectory -PathType Container) {
        foreach ($path in [System.IO.Directory]::EnumerateFiles($licenseDirectory, "*", [System.IO.SearchOption]::AllDirectories)) {
            $paths.Add($path)
        }
    }
    if ($package.license_file) {
        $declaredFile = Join-Path $packageRoot $package.license_file
        if (Test-Path -LiteralPath $declaredFile -PathType Leaf) {
            $paths.Add($declaredFile)
        }
    }

    $documents = [System.Collections.Generic.List[object]]::new()
    foreach ($path in ($paths | Sort-Object -Unique)) {
        try {
            $file = Get-Item -LiteralPath $path
            if ($file.Length -gt $maxLicenseFileBytes) { continue }
            $text = [System.IO.File]::ReadAllText($path, [System.Text.Encoding]::UTF8).Trim()
            $text = [System.Text.RegularExpressions.Regex]::Replace($text, "\r\n?", "`n")
            if ($text) {
                $documents.Add([pscustomobject]@{
                    name = $path.Substring($packageRoot.Length).TrimStart([System.IO.Path]::DirectorySeparatorChar, [System.IO.Path]::AltDirectorySeparatorChar).Replace("\", "/")
                    text = $text
                })
            }
        } catch {
            continue
        }
    }
    return $documents.ToArray()
}

function Get-DocumentSpdxIds([string] $name, [string] $text, [string[]] $declaredIds) {
    $filename = [System.IO.Path]::GetFileName($name)
    $stem = $filename -replace "(?i)\.(md|txt)$", ""
    $normalized = $stem -replace "^(?i:LICENSE|LICENCE)([-._]|$)", ""
    $normalized = $normalized.ToLowerInvariant().Replace("_", "-")
    if ($normalized.StartsWith("licenseref-slint-", [System.StringComparison]::Ordinal)) {
        return ,@($stem)
    }
    if ($filenameIds.ContainsKey($normalized)) {
        $value = $filenameIds[$normalized].Replace(" WITH ", " ")
        return ,@($value -split "\s+")
    }

    $heading = $text.Substring(0, [Math]::Min(1200, $text.Length)).ToLowerInvariant()
    if ($heading.Contains("gnu general public license") -and $declaredIds -contains "GPL-3.0-or-later") {
        return ,@("GPL-3.0-or-later")
    }
    $hints = @(
        @{ id = "MIT"; needles = @("mit license", "permission is hereby granted, free of charge") },
        @{ id = "Apache-2.0"; needles = @("apache license", "licensed under the apache license") },
        @{ id = "BSD-2-Clause"; needles = @("bsd 2-clause") },
        @{ id = "BSD-3-Clause"; needles = @("bsd 3-clause", "neither the name of the copyright holder") },
        @{ id = "BSL-1.0"; needles = @("boost software license") },
        @{ id = "CC0-1.0"; needles = @("creative commons zero", "cc0 1.0 universal") },
        @{ id = "CDLA-Permissive-2.0"; needles = @("community data license agreement", "cdla-permissive") },
        @{ id = "GPL-3.0-only"; needles = @("gnu general public license", "gpl-3.0") },
        @{ id = "ISC"; needles = @("isc license", "permission to use, copy, modify, and/or distribute") },
        @{ id = "MPL-2.0"; needles = @("mozilla public license") },
        @{ id = "Unicode-3.0"; needles = @("unicode, inc. license agreement", "unicode license v3") },
        @{ id = "Unlicense"; needles = @("this is free and unencumbered software") },
        @{ id = "WTFPL"; needles = @("do what the fuck you want to public license") },
        @{ id = "Zlib"; needles = @("zlib license", "this software is provided 'as-is'") }
    )
    foreach ($hint in $hints) {
        foreach ($needle in $hint.needles) {
            if ($heading.Contains($needle)) { return ,@($hint.id) }
        }
    }

    $isLicenseFile = $name -match "^(?i:LICENSE|LICENCE|COPYING)(\.[a-z0-9]+)?$" -or $name -match "^(?i:LICENSES/)"
    if ($isLicenseFile -and $declaredIds.Count -eq 1) { return ,@($declaredIds[0]) }
    return ,@()
}

$packagesById = [System.Collections.Generic.Dictionary[string, object]]::new([System.StringComparer]::Ordinal)
foreach ($target in $targets) {
    $metadataOutput = & cargo metadata --format-version 1 --locked --filter-platform $target
    if ($LASTEXITCODE -ne 0) { throw "cargo metadata failed for target $target" }
    $metadata = ($metadataOutput -join "`n") | ConvertFrom-Json
    foreach ($package in $metadata.packages) {
        if ($package.source) { $packagesById[$package.id] = $package }
    }
}

$packageDocuments = [System.Collections.Generic.Dictionary[string, object]]::new([System.StringComparer]::Ordinal)
$textBySpdx = [System.Collections.Generic.Dictionary[string, object]]::new([System.StringComparer]::Ordinal)
foreach ($entry in ($packagesById.GetEnumerator() | Sort-Object { $_.Value.name.ToLowerInvariant() }, { $_.Value.version }, { $_.Key })) {
    $package = $entry.Value
    $declared = @(Get-SpdxIds $package.license)
    $documents = @(Get-LicenseFiles $package)
    $packageDocuments[$entry.Key] = $documents
    foreach ($document in $documents) {
        foreach ($spdxId in (Get-DocumentSpdxIds $document.name $document.text $declared)) {
            if (-not $textBySpdx.ContainsKey($spdxId)) {
                $textBySpdx[$spdxId] = $document
            }
        }
    }
}

# A number of crates ship only a pointer to the Unlicense, rather than the complete text.
# Bundle the canonical SPDX wording so those rows still expose the full declared option.
$textBySpdx["Unlicense"] = [pscustomobject]@{
    name = "SPDX Unlicense"
    text = @'
This is free and unencumbered software released into the public domain.

Anyone is free to copy, modify, publish, use, compile, sell, or distribute this software, either in source code form or as a compiled binary, for any purpose, commercial or non-commercial, and by any means.

In jurisdictions that recognize copyright laws, the author or authors of this software dedicate any and all copyright interest in the software to the public domain. We make this dedication for the benefit of the public at large and to the detriment of our heirs and successors. We intend this dedication to be an overt act of relinquishment in perpetuity of all present and future rights to this software under copyright law.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

For more information, please refer to <https://unlicense.org/>
'@
}
$textBySpdx["GPL-3.0-or-later"] = [pscustomobject]@{
    name = "SPDX GPL-3.0-or-later"
    text = $textBySpdx["GPL-3.0-only"].text
}
$textBySpdx["WTFPL"] = [pscustomobject]@{
    name = "SPDX WTFPL"
    text = @'
DO WHAT THE FUCK YOU WANT TO PUBLIC LICENSE
Version 2, December 2004

Copyright (C) 2004 Sam Hocevar <sam@hocevar.net>

Everyone is permitted to copy and distribute verbatim or modified copies of this license document, and changing it is allowed as long as the name is changed.

DO WHAT THE FUCK YOU WANT TO PUBLIC LICENSE
TERMS AND CONDITIONS FOR COPYING, DISTRIBUTION AND MODIFICATION

0. You just DO WHAT THE FUCK YOU WANT TO.
'@
}

$uniqueTexts = [ordered]@{}
$catalogPackages = [System.Collections.Generic.List[object]]::new()
foreach ($entry in ($packagesById.GetEnumerator() | Sort-Object { $_.Value.name.ToLowerInvariant() }, { $_.Value.version }, { $_.Key })) {
    $package = $entry.Value
    $declaredExpression = if ($package.license) { $package.license } else { "未声明" }
    $declared = @(Get-SpdxIds $package.license)
    $documents = [System.Collections.Generic.List[object]]::new()
    $localIds = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::Ordinal)

    foreach ($document in $packageDocuments[$entry.Key]) {
        foreach ($spdxId in (Get-DocumentSpdxIds $document.name $document.text $declared)) {
            [void]$localIds.Add($spdxId)
        }
        $documents.Add($document)
    }
    foreach ($spdxId in $declared) {
        if (-not $localIds.Contains($spdxId) -and $textBySpdx.ContainsKey($spdxId)) {
            $document = $textBySpdx[$spdxId]
            $documents.Add([pscustomobject]@{ name = "SPDX $spdxId"; text = $document.text })
            [void]$localIds.Add($spdxId)
        }
    }

    $references = [System.Collections.Generic.List[object]]::new()
    $seenDocuments = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::Ordinal)
    foreach ($document in $documents) {
        $bytes = [System.Text.Encoding]::UTF8.GetBytes($document.text)
        $hash = [System.Security.Cryptography.SHA256]::HashData($bytes)
        $documentId = [Convert]::ToHexString($hash).ToLowerInvariant()
        if (-not $uniqueTexts.Contains($documentId)) { $uniqueTexts[$documentId] = $document.text }
        $referenceKey = "$documentId|$($document.name)"
        if ($seenDocuments.Add($referenceKey)) {
            $references.Add([pscustomobject]@{ id = $documentId; name = $document.name })
        }
    }

    $missingIds = @($declared | Where-Object { -not $localIds.Contains($_) })
    $errorText = $null
    if ($declared.Count -eq 0) {
        $errorText = "此依赖未在 Cargo 清单中声明许可证标识。"
    } elseif ($missingIds.Count -gt 0) {
        $errorText = "无法读取完整许可证正文：" + ($missingIds -join ", ")
    } elseif ($references.Count -eq 0) {
        $errorText = "依赖源包中没有可读取的许可证正文。"
    }

    $catalogPackages.Add([pscustomobject]@{
        id = $entry.Key
        name = $package.name
        version = $package.version
        license = $declaredExpression
        documents = $references.ToArray()
        error = $errorText
    })
}

$catalog = [pscustomobject]@{
    format_version = 1
    targets = $targets
    packages = $catalogPackages.ToArray()
    documents = $uniqueTexts
}
$json = ConvertTo-Json -InputObject $catalog -Depth 12
$outputDirectory = Split-Path -Parent $output
[System.IO.Directory]::CreateDirectory($outputDirectory) | Out-Null
$temporaryOutput = "$output.$PID.tmp"
[System.IO.File]::WriteAllText($temporaryOutput, $json + "`n", [System.Text.UTF8Encoding]::new($false))
[System.IO.File]::Move($temporaryOutput, $output, $true)
$errors = @($catalogPackages | Where-Object { $_.error }).Count
Write-Output "Wrote $($catalogPackages.Count) packages and $($uniqueTexts.Count) unique license texts to assets/third_party_licenses.json ($errors entries need attention)."

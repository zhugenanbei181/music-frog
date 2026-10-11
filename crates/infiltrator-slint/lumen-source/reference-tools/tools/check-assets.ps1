$ErrorActionPreference = "Stop"
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$assetsRoot = Join-Path $repoRoot "assets"
$manifestPath = Join-Path $assetsRoot "manifest.json"
$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json

function Get-PngHeader([string]$Path) {
    $stream = [System.IO.File]::OpenRead($Path)
    try {
        $header = New-Object byte[] 26
        if ($stream.Read($header, 0, $header.Length) -ne $header.Length) {
            return $null
        }
        $width = [System.Net.IPAddress]::NetworkToHostOrder([System.BitConverter]::ToInt32($header, 16))
        $height = [System.Net.IPAddress]::NetworkToHostOrder([System.BitConverter]::ToInt32($header, 20))
        return [pscustomobject]@{
            Width = $width
            Height = $height
            ColorType = $header[25]
        }
    } finally {
        $stream.Dispose()
    }
}

$expectedIcons = @($manifest.icon_set.assets | ForEach-Object { $_.file.Replace("/", "\") })
$duplicateEntries = @($manifest.icon_set.assets | Group-Object file | Where-Object Count -gt 1)
$actualIcons = @(Get-ChildItem -LiteralPath (Join-Path $assetsRoot "icons") -Filter "*.svg" -File | ForEach-Object { "icons\$($_.Name)" })
$iconFileHashes = @(
    $actualIcons | ForEach-Object {
        $relativePath = $_
        [pscustomobject]@{
            File = $relativePath
            Hash = (Get-FileHash -LiteralPath (Join-Path $assetsRoot $relativePath) -Algorithm SHA256).Hash
        }
    }
)
$duplicateContents = @($iconFileHashes | Group-Object Hash | Where-Object Count -gt 1)
$missingIcons = @($expectedIcons | Where-Object { -not (Test-Path -LiteralPath (Join-Path $assetsRoot $_)) })
$unlistedIcons = @($actualIcons | Where-Object { $_ -notin $expectedIcons })
$wrongIconCount = $actualIcons.Count -ne $manifest.icon_set.file_count

$iconReferences = @(
    Select-String -Path (Join-Path $repoRoot "ui\app.slint") -Pattern 'assets/icons/([A-Za-z0-9_-]+\.svg)' -AllMatches |
        ForEach-Object { $_.Matches } |
        ForEach-Object { "icons\$($_.Groups[1].Value)" } |
        Sort-Object -Unique
)
$unknownReferences = @($iconReferences | Where-Object { $_ -notin $expectedIcons })
$unusedIcons = @($expectedIcons | Where-Object { $_ -notin $iconReferences })

$flagFiles = @(Get-ChildItem -LiteralPath (Join-Path $assetsRoot "flags") -Filter "*.svg" -File | Sort-Object Name)
$flagFileCodes = @($flagFiles | ForEach-Object { $_.BaseName })
$runtimeFlagDirectory = Join-Path $assetsRoot "flags\circle-png"
$runtimeFlagFiles = @(Get-ChildItem -LiteralPath $runtimeFlagDirectory -Filter "*.png" -File -ErrorAction SilentlyContinue | Sort-Object Name)
$runtimeFlagCodes = @($runtimeFlagFiles | ForEach-Object { $_.BaseName })
$slintFlagCodes = @(
    [regex]::Matches((Get-Content (Join-Path $repoRoot "ui\flag-icons.slint") -Raw), 'assets/flags/circle-png/([a-z]{2})\.png') |
        ForEach-Object { $_.Groups[1].Value }
)
$rustFlagCodes = @(
    [regex]::Matches((Get-Content (Join-Path $repoRoot "src\flag_assets.rs") -Raw), '"([a-z]{2})"\s*=>\s*Some\(\d+\)') |
        ForEach-Object { $_.Groups[1].Value }
)
$flagOrderMismatch = ([string]::Join(",", $flagFileCodes) -ne [string]::Join(",", $runtimeFlagCodes)) -or
    ([string]::Join(",", $flagFileCodes) -ne [string]::Join(",", $slintFlagCodes)) -or
    ([string]::Join(",", $flagFileCodes) -ne [string]::Join(",", $rustFlagCodes))
$wrongFlagCount = $flagFiles.Count -ne $manifest.flag_set.file_count -or $runtimeFlagFiles.Count -ne $manifest.flag_set.file_count
$invalidFlagNames = @($flagFiles | Where-Object { $_.BaseName -notmatch '^[a-z]{2}$' })
$invalidRuntimeFlagNames = @($runtimeFlagFiles | Where-Object { $_.BaseName -notmatch '^[a-z]{2}$' })
$invalidRuntimeFlags = @(
    $runtimeFlagFiles | Where-Object {
        $header = Get-PngHeader $_.FullName
        $null -eq $header -or $header.Width -ne 144 -or $header.Height -ne 144 -or $header.ColorType -ne 6
    } | ForEach-Object { $_.Name }
)

$actualRasterFiles = @(
    Get-ChildItem -LiteralPath $assetsRoot -Recurse -File |
        Where-Object { $_.Extension -match '^\.(png|jpe?g|webp)$' } |
        ForEach-Object { $_.FullName.Substring($assetsRoot.Length + 1).Replace("\", "/") } |
        Where-Object { $_ -notlike "flags/circle-png/*" }
)
$declaredRasterFiles = @($manifest.raster_assets)
$unlistedRasterFiles = @($actualRasterFiles | Where-Object { $_ -notin $declaredRasterFiles })
$missingRasterFiles = @($declaredRasterFiles | Where-Object { -not (Test-Path -LiteralPath (Join-Path $assetsRoot $_)) })

$errors = @()
if ($duplicateEntries.Count -gt 0) { $errors += "Manifest has duplicate icon entries: $($duplicateEntries.Name -join ', ')" }
if ($duplicateContents.Count -gt 0) {
    $duplicateFiles = @($duplicateContents | ForEach-Object { ($_.Group | ForEach-Object File) -join ", " })
    $errors += "UI icons contain duplicate SVG files: $($duplicateFiles -join '; ')"
}
if ($missingIcons.Count -gt 0) { $errors += "Missing icon files: $($missingIcons -join ', ')" }
if ($unlistedIcons.Count -gt 0) { $errors += "Unlisted SVG files: $($unlistedIcons -join ', ')" }
if ($wrongIconCount) { $errors += "Manifest expects $($manifest.icon_set.file_count) functional SVGs; found $($actualIcons.Count)." }
if ($unknownReferences.Count -gt 0) { $errors += "UI references missing from manifest: $($unknownReferences -join ', ')" }
if ($unusedIcons.Count -gt 0) { $errors += "Manifest icons with no UI reference: $($unusedIcons -join ', ')" }
if ($wrongFlagCount) { $errors += "Manifest expects $($manifest.flag_set.file_count) source SVGs and circular runtime PNGs; found $($flagFiles.Count) SVGs and $($runtimeFlagFiles.Count) PNGs." }
if ($invalidFlagNames.Count -gt 0) { $errors += "Flag names must be two lowercase letters: $($invalidFlagNames.Name -join ', ')" }
if ($invalidRuntimeFlagNames.Count -gt 0) { $errors += "Runtime flag PNG names must match their two-letter SVG source codes: $($invalidRuntimeFlagNames.Name -join ', ')" }
if ($invalidRuntimeFlags.Count -gt 0) { $errors += "Runtime flags must be 144x144 RGBA PNGs: $($invalidRuntimeFlags -join ', ')" }
if ($flagOrderMismatch) { $errors += "Flag SVGs, ui/flag-icons.slint and src/flag_assets.rs must have the same ordered code list." }
if ($unlistedRasterFiles.Count -gt 0) { $errors += "Raster images missing from manifest: $($unlistedRasterFiles -join ', ')" }
if ($missingRasterFiles.Count -gt 0) { $errors += "Manifest raster files are missing: $($missingRasterFiles -join ', ')" }

if ($errors.Count -gt 0) { throw ($errors -join [Environment]::NewLine) }
Write-Output "Asset catalog OK: $($expectedIcons.Count) UI SVGs and $($runtimeFlagFiles.Count) ordered circular flag PNGs."

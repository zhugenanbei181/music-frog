param(
    [string]$OutputDirectory = "target/ui-preview/final-matrix"
)

$ErrorActionPreference = "Stop"
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
Set-Location -LiteralPath $repoRoot

$outputPath = Join-Path $repoRoot $OutputDirectory
New-Item -ItemType Directory -Force -Path $outputPath | Out-Null

$sizes = @(
    @{ Width = 320; Height = 720 },
    @{ Width = 360; Height = 800 },
    @{ Width = 400; Height = 880 },
    @{ Width = 480; Height = 960 }
)

$scenarios = @()
foreach ($size in $sizes) {
    foreach ($variant in @("app-page", "app-page-dark-debug", "app-user-agent-debug", "app-user-agent-dark-debug", "app-theme-color-debug", "app-page-scale-debug")) {
        $scenarios += @{ Page = "AppSettings"; Width = $size.Width; Height = $size.Height; Variant = $variant }
    }
}
foreach ($section in 0..5) {
    foreach ($variant in @("app-section-$section", "app-section-$section-debug")) {
        $scenarios += @{ Page = "AppSettings"; Width = 400; Height = 880; Variant = $variant }
    }
}
$scenarios += @{ Page = "AppSettings"; Width = 320; Height = 720; Variant = "app-user-agent-error-debug" }
$scenarios += @{ Page = "AppSettings"; Width = 400; Height = 880; Variant = "app-page-english" }
foreach ($size in $sizes) {
    foreach ($page in @("Home", "Config", "Settings")) {
        $scenarios += @{ Page = $page; Width = $size.Width; Height = $size.Height; Variant = "" }
    }
    $scenarios += @{ Page = "Proxy"; Width = $size.Width; Height = $size.Height; Variant = "groups" }
}
$scenarios += @{ Page = "Proxy"; Width = 320; Height = 720; Variant = "detail-light" }
$scenarios += @{ Page = "Proxy"; Width = 320; Height = 720; Variant = "detail-dark" }
$scenarios += @{ Page = "Proxy"; Width = 400; Height = 880; Variant = "detail-light" }
$scenarios += @{ Page = "Proxy"; Width = 400; Height = 880; Variant = "detail-dark" }
$scenarios += @{ Page = "Proxy"; Width = 400; Height = 880; Variant = "proxy-ping-states" }
$scenarios += @{ Page = "Proxy"; Width = 400; Height = 880; Variant = "proxy-ping-loading" }
$scenarios += @{ Page = "Preview"; Width = 320; Height = 720; Variant = "" }
$scenarios += @{ Page = "Preview"; Width = 400; Height = 880; Variant = "" }
$scenarios += @{ Page = "Meta"; Width = 320; Height = 720; Variant = "" }
$scenarios += @{ Page = "Meta"; Width = 400; Height = 880; Variant = "" }
foreach ($size in $sizes) {
    $scenarios += @{ Page = "Config"; Width = $size.Width; Height = $size.Height; Variant = "add-profile-light" }
    $scenarios += @{ Page = "Config"; Width = $size.Width; Height = $size.Height; Variant = "add-profile-dark" }
}
$scenarios += @{ Page = "Home"; Width = 320; Height = 720; Variant = "layout-debug" }
$scenarios += @{ Page = "Config"; Width = 400; Height = 880; Variant = "layout-debug-sheet" }
$scenarios += @{ Page = "Config"; Width = 400; Height = 880; Variant = "config-sheet-debug" }
foreach ($variant in @("config-action-debug", "config-share-debug", "config-delete-debug", "config-settings-debug", "config-settings-overrides-debug")) {
    $scenarios += @{ Page = "Config"; Width = 400; Height = 880; Variant = $variant }
}
foreach ($size in $sizes) {
    foreach ($variant in @("home-stopped", "home-empty", "home-stopped-debug")) {
        $scenarios += @{ Page = "Home"; Width = $size.Width; Height = $size.Height; Variant = $variant }
    }
}

& cargo build --quiet --manifest-path "tools/ui-preview/Cargo.toml"
if ($LASTEXITCODE -ne 0) { throw "UI preview build failed" }
$previewMetadata = & cargo metadata --no-deps --format-version 1 --manifest-path "tools/ui-preview/Cargo.toml" | ConvertFrom-Json
$previewExecutable = Join-Path $previewMetadata.target_directory "debug/lumen-ui-preview.exe"
foreach ($scenario in $scenarios) {
    $variantLabel = switch ($scenario.Variant) {
        "groups" { "Groups" }
        "detail-light" { "DetailLight" }
        "detail-dark" { "DetailDark" }
        "proxy-ping-states" { "PingStates" }
        "proxy-ping-loading" { "PingLoading" }
        "add-profile-light" { "AddProfileLight" }
        "add-profile-dark" { "AddProfileDark" }
        "layout-debug" { "LayoutDebug" }
        "layout-debug-sheet" { "LayoutDebugSheet" }
        "home-stopped" { "Stopped" }
        "home-empty" { "Empty" }
        "home-stopped-debug" { "StoppedDebug" }
        "config-sheet-debug" { "ConfigSheetDebug" }
        "config-action-debug" { "EditActionDebug" }
        "config-share-debug" { "ShareDebug" }
        "config-delete-debug" { "DeleteDebug" }
        "config-settings-debug" { "ProfileSettingsDebug" }
        "config-settings-overrides-debug" { "ProfileSettingsOverridesDebug" }
        default { if ($scenario.Variant.StartsWith("app-")) { $scenario.Variant } else { "" } }
    }
    if ($variantLabel) {
        $fileName = "{0}{1}-{2}x{3}.png" -f $scenario.Page, $variantLabel, $scenario.Width, $scenario.Height
    } else {
        $fileName = "{0}-{1}x{2}.png" -f $scenario.Page, $scenario.Width, $scenario.Height
    }
    $snapshot = Join-Path $outputPath $fileName
    & $previewExecutable $scenario.Page $scenario.Width $scenario.Height $snapshot $scenario.Variant
    if ($LASTEXITCODE -ne 0) {
        throw "UI snapshot failed: $fileName"
    }
}

Write-Host "Rendered $($scenarios.Count) UI snapshots to $outputPath."

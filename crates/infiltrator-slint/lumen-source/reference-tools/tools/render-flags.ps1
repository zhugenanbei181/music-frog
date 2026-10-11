param(
    [int]$Size = 144,
    [int]$ThrottleLimit = 8
)

$ErrorActionPreference = "Stop"
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
Set-Location -LiteralPath $repoRoot

& cargo run --quiet --manifest-path "tools/flag-renderer/Cargo.toml" -- $repoRoot
if ($LASTEXITCODE -ne 0) {
    throw "Circular flag asset generation failed."
}

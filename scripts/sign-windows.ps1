<#
.SYNOPSIS
  Signs Windows executables/installers using Authenticode and RFC3161 timestamps with automatic server fallback.

.PARAMETER TargetPath
  Path to the .exe, .msi, or .dll to be signed.

.PARAMETER CertPath
  Path to the .pfx certificate file (if not using certificate store).

.PARAMETER CertPassword
  Password for the .pfx certificate file.
#>

param (
    [Parameter(Mandatory=$true)]
    [string]$TargetPath,

    [Parameter(Mandatory=$false)]
    [string]$CertPath = $env:WINDOWS_CERT_PATH,

    [Parameter(Mandatory=$false)]
    [string]$CertPassword = $env:WINDOWS_CERT_PASSWORD,

    [Parameter(Mandatory=$false)]
    [string]$CertThumbprint = $env:WINDOWS_CERT_THUMBPRINT
)

$ErrorActionPreference = "Stop"

if (-not (Test-Path $TargetPath)) {
    Write-Error "Target file not found: $TargetPath"
    exit 1
}

# Timestamp servers to try sequentially on network / transient failures
$TimestampServers = @(
    "http://timestamp.digicert.com",
    "http://timestamp.sectigo.com",
    "http://tsa.starfieldtech.com",
    "http://timestamp.globalsign.com/scripts/timstamp.dll"
)

# Locate signtool.exe
$SignTool = Get-Command "signtool.exe" -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Source
if (-not $SignTool) {
    $KitPaths = @(
        "C:\Program Files (x86)\Windows Kits\10\bin\*\x64\signtool.exe",
        "C:\Program Files\Windows Kits\10\bin\*\x64\signtool.exe"
    )
    $SignTool = Resolve-Path $KitPaths -ErrorAction SilentlyContinue | Select-Object -Last 1 -ExpandProperty Path
}

if (-not $SignTool) {
    Write-Error "signtool.exe not found in PATH or Windows Kits directories"
    exit 1
}

Write-Host "[SignWindows] Using SignTool: $SignTool"
Write-Host "[SignWindows] Signing target: $TargetPath"

$Signed = $false
foreach ($TSA in $TimestampServers) {
    Write-Host "[SignWindows] Attempting signature with TSA: $TSA"

    $SignArgs = @("sign", "/fd", "SHA256", "/tr", $TSA, "/td", "SHA256", "/v")

    if ($CertThumbprint) {
        $SignArgs += @("/sha1", $CertThumbprint, "/sm")
    } elseif ($CertPath -and (Test-Path $CertPath)) {
        $SignArgs += @("/f", $CertPath)
        if ($CertPassword) {
            $SignArgs += @("/p", $CertPassword)
        }
    } else {
        # Auto-detect installed code signing certificate
        $SignArgs += @("/a")
    }

    $SignArgs += $TargetPath

    $Process = Start-Process -FilePath $SignTool -ArgumentList $SignArgs -NoNewWindow -Wait -PassThru
    if ($Process.ExitCode -eq 0) {
        Write-Host "[SignWindows] Successfully signed with $TSA"
        $Signed = $true
        break
    } else {
        Write-Warning "[SignWindows] Signing failed with $TSA (exit code: $($Process.ExitCode)), retrying with next TSA..."
        Start-Sleep -Seconds 2
    }
}

if (-not $Signed) {
    Write-Error "[SignWindows] All signing attempts failed"
    exit 1
}

# Verify Signature
Write-Host "[SignWindows] Verifying Authenticode signature..."
$VerifyArgs = @("verify", "/pa", "/v", $TargetPath)
$VerifyProc = Start-Process -FilePath $SignTool -ArgumentList $VerifyArgs -NoNewWindow -Wait -PassThru
if ($VerifyProc.ExitCode -ne 0) {
    Write-Error "[SignWindows] Verification failed (exit code: $($VerifyProc.ExitCode))"
    exit 1
}

Write-Host "[SignWindows] Verification PASSED: $TargetPath"

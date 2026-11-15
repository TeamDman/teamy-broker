#requires -Version 7.0
[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $false

function Invoke-NativeCheck {
    param([string]$Label, [string]$Executable, [string[]]$Arguments)
    Write-Host -ForegroundColor Yellow "Running $Label..."
    & $Executable @Arguments
    if ($LASTEXITCODE -ne 0) { throw "$Label failed with exit code $LASTEXITCODE" }
}

function Invoke-ScriptCheck {
    param([string]$Label, [string]$Script, [hashtable]$Arguments = @{})
    Write-Host -ForegroundColor Yellow "Running $Label..."
    # Each checked-in script throws on failure. Do not inspect a stale exit code
    # left by a native command from an earlier, unrelated step.
    & (Join-Path $PSScriptRoot $Script) @Arguments
}

Push-Location -LiteralPath $PSScriptRoot
try {
    $metadataText = & cargo metadata --offline --locked --no-deps --format-version 1
    if ($LASTEXITCODE -ne 0) { throw 'Cargo metadata failed before validation.' }
    $metadata = $metadataText | ConvertFrom-Json
    $manifestPath = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot 'Cargo.toml'))
    $package = $metadata.packages | Where-Object { [IO.Path]::GetFullPath($_.manifest_path) -eq $manifestPath } | Select-Object -First 1
    if (-not $package) { throw 'Could not determine the broker package from Cargo metadata.' }
    $features = @($package.features.PSObject.Properties.Name | Where-Object { $_ -notin @('default', 'tracy') })
    $testFeatures = if ($features.Count) { @('--features', ($features -join ',')) } else { @() }

    Invoke-ScriptCheck 'validation wrapper fixtures' 'tests/tooling/check-runner.tests.ps1'
    Invoke-ScriptCheck 'publication policy fixtures' 'tests/tooling/publication-policy.tests.ps1'
    Invoke-ScriptCheck 'publication scan' 'check-publication.ps1'
    Invoke-NativeCheck 'format check' 'rustup' @('run', 'nightly', '--', 'cargo', 'fmt', '--all', '--', '--check')
    Invoke-NativeCheck 'workspace Clippy' 'cargo' @('clippy', '--offline', '--locked', '--workspace', '--all-targets', '--all-features', '--no-deps', '--', '-D', 'warnings')
    Invoke-NativeCheck 'isolated SDK dependency check' 'cargo' @('check', '--offline', '--locked', '--package', 'teamy-broker-sdk', '--all-targets')
    Invoke-ScriptCheck 'broker lint policy fixtures' 'check-lint-policy.ps1'
    Invoke-ScriptCheck 'SDK lint policy fixtures' 'check-lint-policy.ps1' @{ ManifestPath = (Join-Path $PSScriptRoot 'sdk/Cargo.toml') }

    # Existing pinned dependencies and a JDK are prerequisites. These host checks
    # do not download tools, accept licenses, sign APKs or install on a device.
    Invoke-ScriptCheck 'pinned pairing Java contracts' 'pairing/java/build.ps1' @{ Test = $true }
    Invoke-ScriptCheck 'Android host policy contracts' 'android/check-java.ps1'
    Invoke-NativeCheck 'workspace build' 'cargo' @('build', '--offline', '--locked', '--workspace', '--all-features', '--quiet')
    Invoke-NativeCheck 'test example build' 'cargo' (@('build', '--offline', '--locked', '--examples', '--quiet') + $testFeatures)
    Invoke-ScriptCheck 'Android/Rust loopback TLS contracts' 'android/check-tailnet-transport.ps1' @{ TargetDirectory = $metadata.target_directory }
    Invoke-ScriptCheck 'Windows executable manifest' 'resources/check-windows-manifest.ps1' @{ Executable = (Join-Path $metadata.target_directory 'debug/teamy-broker.exe') }
    Invoke-NativeCheck 'workspace tests' 'cargo' (@('test', '--offline', '--locked', '--workspace', '--quiet') + $testFeatures)
} finally {
    Pop-Location
}

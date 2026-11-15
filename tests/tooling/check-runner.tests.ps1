# Validation-wrapper contracts using PowerShell functions as command doubles.
# No native tools, compiler, profiler or Java process is started.
$ErrorActionPreference = 'Stop'
$repository = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '../..')).Path
$checkAll = Join-Path $repository 'check-all.ps1'
$savedExit = $global:LASTEXITCODE
$fixtureCapture = @{ Directory = $null }

function cargo {
    $fixtureCapture.Directory = (Get-Location).Path
    $global:LASTEXITCODE = 23
}

Push-Location -LiteralPath ([IO.Path]::GetTempPath())
try {
    $callerDirectory = (Get-Location).Path
    $failed = $false
    try { & $checkAll } catch {
        if ($_.Exception.Message -ne 'Cargo metadata failed before validation.') { throw }
        $failed = $true
    }
    if (-not $failed) { throw 'Metadata failure did not stop validation.' }
    if ($fixtureCapture.Directory -ne $repository) { throw 'Validation ran in the caller directory.' }
    if ((Get-Location).Path -ne $callerDirectory) { throw 'Validation did not restore the caller directory on failure.' }
} finally {
    Pop-Location
    Remove-Item -LiteralPath Function:cargo
    $global:LASTEXITCODE = $savedExit
}

$tokens = $null
$errors = $null
$ast = [System.Management.Automation.Language.Parser]::ParseFile($checkAll, [ref]$tokens, [ref]$errors)
$nativeCheck = $ast.Find({ param($node) $node -is [System.Management.Automation.Language.FunctionDefinitionAst] -and $node.Name -eq 'Invoke-NativeCheck' }, $true)
if (-not $nativeCheck) { throw 'Native-check wrapper is missing.' }
. ([scriptblock]::Create($nativeCheck.Extent.Text))
function Invoke-FixtureFailure { $global:LASTEXITCODE = 37 }
$continued = $false
$failed = $false
try {
    try {
        Invoke-NativeCheck 'failure fixture' 'Invoke-FixtureFailure' @()
        $continued = $true
    } catch {
        if ($_.Exception.Message -ne 'failure fixture failed with exit code 37') { throw }
        $failed = $true
    }
    if (-not $failed -or $continued) { throw 'A later successful step could mask a failed native command.' }
} finally {
    $global:LASTEXITCODE = $savedExit
}

$rejected = $false
try { & (Join-Path $repository 'run-profiler.ps1') } catch {
    if ($_.Exception.Message -notlike 'Supply the broker command to profile*') { throw }
    $rejected = $true
}
if (-not $rejected) { throw 'Profiler accepted an implicit command.' }
Write-Output 'Validation wrapper fixtures passed: root selection, failure propagation, caller restoration and explicit profiling command.'

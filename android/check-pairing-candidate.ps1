param([switch]$Execute, [string]$SdkRoot, [string]$DebugKeystore, [string]$JavaBin)
$ErrorActionPreference='Stop'
$repo=(Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$candidate=[IO.Path]::GetFullPath((Join-Path $repo 'target/mobile-pairing-candidate-20261010'))
$prefix=[IO.Path]::GetFullPath((Join-Path $repo 'target'))+[IO.Path]::DirectorySeparatorChar
if(!$candidate.StartsWith($prefix,[StringComparison]::OrdinalIgnoreCase)){throw 'Candidate must remain inside repository target directory.'}
if(!$Execute){
    Write-Output ('PROPOSAL ONLY: sequential offline Cargo checks, jobs2, candidate target '+$candidate)
    Write-Output 'No build starts. Requires serialized slot. No live broker executable/process/lease is changed.'
    return
}
if (!$SdkRoot -or !$DebugKeystore -or !$JavaBin) { throw 'SDK, debug keystore, and JDK paths are required.' }
$drive=Get-PSDrive -Name ([IO.Path]::GetPathRoot($candidate).Substring(0,1))
if($drive.Free -lt 10GB){throw 'Insufficient free space; stop without pruning.'}
function Invoke-CandidateCargo([string[]]$Arguments){
    & cargo @Arguments
    if($LASTEXITCODE){throw 'Candidate check failed; stop without modifying live output or pruning.'}
}
Push-Location -LiteralPath $repo
try{
    # Rebuild repaired Java before Rust tests; never test against stale rejected 0.4 jars.
    & (Join-Path $repo 'pairing/java/build.ps1') -Test
    & (Join-Path $repo 'android/check-java.ps1')
    & (Join-Path $repo 'android/build-debug.ps1') -SdkRoot $SdkRoot -DebugKeystore $DebugKeystore -JavaBin $JavaBin
    # Build all examples first: the isolated autostart contract consumes synthetic helper executables.
    Invoke-CandidateCargo @('build','--offline','-j2','--target-dir',$candidate,'--workspace','--bins','--examples')
    Invoke-CandidateCargo @('test','--offline','-j2','--target-dir',$candidate,'--workspace','--','--test-threads=1')
    Invoke-CandidateCargo @('clippy','--offline','-j2','--target-dir',$candidate,'--workspace','--all-targets','--','-D','warnings')
    $artifact=Join-Path $candidate 'debug/examples/mobile_handoff_pairing.exe'
    Write-Output ('Broker candidate: '+(Join-Path $candidate 'debug/teamy-broker.exe'))
    Write-Output ('Broker candidate SHA256: '+(Get-FileHash -LiteralPath (Join-Path $candidate 'debug/teamy-broker.exe') -Algorithm SHA256).Hash)
    Write-Output ('Inert desktop artifact: '+$artifact)
    Write-Output ('Desktop SHA256: '+(Get-FileHash -LiteralPath $artifact -Algorithm SHA256).Hash)
    Write-Output ('APK SHA256: '+(Get-FileHash -LiteralPath (Join-Path $repo 'android/build/debug/teamy-broker-DUMMY-ONLY-debug.apk') -Algorithm SHA256).Hash)
}finally{Pop-Location}

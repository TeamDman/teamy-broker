param(
    [Parameter(Mandatory)][string]$BinaryPath,
    [Parameter(Mandatory)][string]$OutputDirectory
)
$ErrorActionPreference='Stop'
$root=$PSScriptRoot
$pairing=Join-Path $root 'pairing/java'
$destination=$ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($OutputDirectory)
if(Test-Path -LiteralPath $destination){throw 'Package destination already exists; choose a new directory.'}

function Get-ContainedPath([string]$Base,[string]$Relative) {
    if([IO.Path]::IsPathRooted($Relative)){throw 'Package input path must be relative.'}
    $basePath=[IO.Path]::GetFullPath($Base).TrimEnd([IO.Path]::DirectorySeparatorChar)+[IO.Path]::DirectorySeparatorChar
    $candidate=[IO.Path]::GetFullPath((Join-Path $basePath $Relative))
    if(!$candidate.StartsWith($basePath,[StringComparison]::OrdinalIgnoreCase)){throw 'Package input escaped its directory.'}
    return $candidate
}
function Assert-Hash([string]$Path,[string]$Expected) {
    if($Expected -notmatch '^[0-9A-F]{64}$' -or !(Test-Path -LiteralPath $Path -PathType Leaf) -or (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash -cne $Expected){throw 'Package input is missing or mismatched.'}
}

$receiptPath=Join-Path $pairing 'build/runtime-artifacts.json'
$receiptHash=(Get-FileHash -LiteralPath $receiptPath -Algorithm SHA256).Hash
$receipt=Get-Content -Raw -LiteralPath $receiptPath | ConvertFrom-Json
if($receipt.SchemaVersion -ne 1){throw 'Build pairing/java/build.ps1 before packaging.'}
$expectedInputs=@('build.ps1','dependencies-lock.json','vendor-lock.json')
$expectedInputs+=@(Get-ChildItem -LiteralPath (Join-Path $pairing 'src'),(Join-Path $pairing 'desktop') -Recurse -Filter '*.java' | ForEach-Object {[IO.Path]::GetRelativePath($pairing,$_.FullName).Replace('\','/')})
if((@($expectedInputs | Sort-Object) -join "`n") -cne (@($receipt.Inputs.File | Sort-Object) -join "`n")){throw 'Pairing source inventory changed; rebuild before packaging.'}
foreach($inputRecord in $receipt.Inputs){Assert-Hash (Get-ContainedPath $pairing $inputRecord.File) $inputRecord.SHA256}
$artifactNames=@('ukey2-runtime.jar','ukey2-worker.jar')
if((@($receipt.Artifacts.File | Sort-Object) -join ',') -cne ($artifactNames -join ',')){throw 'Pairing artifact receipt rejected.'}
foreach($artifact in $receipt.Artifacts){Assert-Hash (Get-ContainedPath (Join-Path $pairing 'build') $artifact.File) $artifact.SHA256}

$dependencyLock=Get-Content -Raw -LiteralPath (Join-Path $pairing 'dependencies-lock.json') | ConvertFrom-Json
$runtimeNames=@('protobuf-java-3.25.9.jar','guava-33.7.2-android.jar','jsr305-3.0.2.jar','failureaccess-1.0.3.jar')
foreach($name in $runtimeNames) {
    $record=@($dependencyLock | Where-Object File -CEQ $name)
    if($record.Count -ne 1){throw 'Pinned runtime dependency record missing or duplicated.'}
    Assert-Hash (Join-Path $pairing ('deps/'+$name)) $record[0].SHA256
}
$vendorLock=Get-Content -Raw -LiteralPath (Join-Path $pairing 'vendor-lock.json') | ConvertFrom-Json
foreach($record in $vendorLock.Files){Assert-Hash (Get-ContainedPath (Join-Path $pairing 'vendor/google-ukey2') $record.Path) $record.SHA256}
$licenseLockHash=(Get-FileHash -LiteralPath (Join-Path $pairing 'runtime-licenses-lock.json') -Algorithm SHA256).Hash
$licenseLock=Get-Content -Raw -LiteralPath (Join-Path $pairing 'runtime-licenses-lock.json') | ConvertFrom-Json
if((@($licenseLock.File | Sort-Object) -join ',') -cne 'jsr305-LICENSE,protobuf-LICENSE'){throw 'Runtime license inventory rejected.'}
foreach($record in $licenseLock){Assert-Hash (Get-ContainedPath (Join-Path $pairing 'licenses') $record.File) $record.SHA256}

$copies=[Collections.Generic.List[object]]::new()
function Add-Copy([string]$Source,[string]$Relative,[string]$Expected) {
    $file=Get-Item -LiteralPath $Source
    if($file.PSIsContainer){throw 'Package input must be a file.'}
    $hash=(Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash
    if($Expected -and $hash -cne $Expected){throw 'Verified input changed before packaging.'}
    $copies.Add([pscustomobject]@{Source=$file.FullName;File=$Relative;SHA256=$hash;Bytes=$file.Length})
}
Add-Copy $BinaryPath 'teamy-broker.exe'
Add-Copy (Join-Path $root 'LICENSE') 'LICENSE'
foreach($artifact in $receipt.Artifacts){Add-Copy (Join-Path $pairing ('build/'+$artifact.File)) ('pairing/build/'+$artifact.File) $artifact.SHA256}
Add-Copy $receiptPath 'pairing/build/runtime-artifacts.json' $receiptHash
foreach($name in $runtimeNames){Add-Copy (Join-Path $pairing ('deps/'+$name)) ('pairing/deps/'+$name) ($dependencyLock | Where-Object File -CEQ $name).SHA256}
foreach($name in @('dependencies-lock.json','vendor-lock.json')){Add-Copy (Join-Path $pairing $name) ('pairing/'+$name) ($receipt.Inputs | Where-Object File -CEQ $name).SHA256}
Add-Copy (Join-Path $pairing 'runtime-licenses-lock.json') 'pairing/runtime-licenses-lock.json' $licenseLockHash
foreach($name in @('LICENSE','NOTICE')){Add-Copy (Join-Path $pairing ('vendor/google-ukey2/'+$name)) ('pairing/licenses/google-ukey2/'+$name) ($vendorLock.Files | Where-Object Path -CEQ $name).SHA256}
foreach($record in $licenseLock){Add-Copy (Join-Path $pairing ('licenses/'+$record.File)) ('pairing/licenses/'+$record.File) $record.SHA256}

# Inputs are checked before creating the fresh destination. No deployment is overwritten.
New-Item -ItemType Directory -Path $destination | Out-Null
foreach($copy in $copies) {
    $target=Get-ContainedPath $destination $copy.File
    New-Item -ItemType Directory -Path (Split-Path $target) -Force | Out-Null
    Copy-Item -LiteralPath $copy.Source -Destination $target
    Assert-Hash $target $copy.SHA256
}
$files=@($copies | ForEach-Object {[ordered]@{File=$_.File;SHA256=$_.SHA256;Bytes=$_.Bytes}})
[ordered]@{SchemaVersion=1;Files=$files} | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $destination 'package-manifest.json') -Encoding utf8
Get-FileHash -LiteralPath (Join-Path $destination 'package-manifest.json') -Algorithm SHA256
Write-Output 'Prepared standalone broker package with verified pairing runtime.'

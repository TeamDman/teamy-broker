param(
    [Parameter(Mandatory)][string]$SdkRoot,
    [Parameter(Mandatory)][string]$DebugKeystore,
    [string]$JavaBin = (Split-Path (Get-Command javac -ErrorAction Stop).Source),
    [string]$ManifestPath = (Join-Path $PSScriptRoot 'AndroidManifest.xml'),
    [string]$OutputDirectory = (Join-Path $PSScriptRoot 'build/debug'),
    [string]$SigningAlias = 'androiddebugkey',
    [string]$SigningPasswordEnvironment,
    [string]$KeyPasswordEnvironment,
    [string]$ExpectedSignerSha256
)
$ErrorActionPreference = 'Stop'
# Remove signing passwords from the parent environment before compilation/tools.
$signingChildEnvironment = @{}
foreach($name in @($SigningPasswordEnvironment,$KeyPasswordEnvironment) | Where-Object { $_ } | Select-Object -Unique) {
    if($name -notmatch '^[A-Za-z_][A-Za-z0-9_]*$') { throw 'Invalid password environment variable name.' }
    $value = [Environment]::GetEnvironmentVariable($name,'Process')
    if([string]::IsNullOrEmpty($value)) { throw 'Signing password environment unavailable.' }
    $signingChildEnvironment[$name] = $value
    [Environment]::SetEnvironmentVariable($name,$null,'Process')
}
function Run-SigningTool([string]$Executable, [string[]]$Arguments) {
    $info = [Diagnostics.ProcessStartInfo]::new($Executable)
    $info.UseShellExecute = $false
    foreach($arg in $Arguments) { $info.ArgumentList.Add($arg) }
    foreach($name in $signingChildEnvironment.Keys) { $info.Environment[$name] = $signingChildEnvironment[$name] }
    try {
        $child = [Diagnostics.Process]::Start($info)
        $child.WaitForExit()
        if($child.ExitCode) { throw 'APK signing failed.' }
        $child.Dispose()
    } finally { $signingChildEnvironment.Clear() }
}

# This script never downloads tools, accepts licenses, creates keys, or installs on a device.
$sdk = (Resolve-Path -LiteralPath $SdkRoot).Path
$key = (Resolve-Path -LiteralPath $DebugKeystore).Path
$platform = Get-ChildItem -LiteralPath (Join-Path $sdk 'platforms') -Directory |
    Where-Object { $_.Name -match '^android-(35|36)$' } | Sort-Object Name -Descending | Select-Object -First 1
$tools = Get-ChildItem -LiteralPath (Join-Path $sdk 'build-tools') -Directory |
    Where-Object { Test-Path -LiteralPath (Join-Path $_.FullName 'lib/d8.jar') } | Sort-Object { [version]($_.Name -replace '-.*','') } -Descending | Select-Object -First 1
if (!$platform -or !$tools) { throw 'Approved SDK platform 35/36 and build-tools with D8 are required.' }
$androidJar = Join-Path $platform.FullName 'android.jar'
[xml]$manifest = [IO.File]::ReadAllText((Resolve-Path -LiteralPath $ManifestPath).Path)
$minimumSdk = $manifest.manifest.'uses-sdk'.GetAttribute('minSdkVersion','http://schemas.android.com/apk/res/android')
$minimumApi = 0
if (![int]::TryParse($minimumSdk,[ref]$minimumApi) -or $minimumApi -lt 1) { throw 'Manifest must declare a numeric minimum SDK.' }
$out = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($OutputDirectory)
$work = Join-Path $out ('run-'+[Guid]::NewGuid().ToString('N'))
$classes = Join-Path $work 'classes'
$dex = Join-Path $work 'dex'
try {
New-Item -ItemType Directory -Path $classes,$dex -Force | Out-Null
function Run-Tool([string]$Executable, [string[]]$Arguments) {
    & $Executable @Arguments
    if ($LASTEXITCODE) { throw "Build tool failed: $([System.IO.Path]::GetFileName($Executable))" }
}
$pairingRoot = Join-Path $PSScriptRoot '../pairing/java'
$pairingJar = Join-Path $pairingRoot 'build/ukey2-runtime.jar'
if (!(Test-Path -LiteralPath $pairingJar)) { throw 'Build pinned pairing/java/build.ps1 first; no downloads.' }
$runtimeNames = @('protobuf-java-3.25.9.jar','guava-33.7.2-android.jar','jsr305-3.0.2.jar','failureaccess-1.0.3.jar')
$runtime = @($runtimeNames | ForEach-Object {Join-Path $pairingRoot ('deps/'+$_)})
$classpath = (@($androidJar,$pairingJar)+$runtime) -join ';'
$sources = @(Get-ChildItem -LiteralPath (Join-Path $PSScriptRoot 'src') -Recurse -Filter '*.java' | Select-Object -ExpandProperty FullName)
Run-Tool (Join-Path $JavaBin 'javac.exe') (@('-encoding','UTF-8','-source','8','-target','8','-classpath',$classpath,'-d',$classes) + $sources)
$classJar = Join-Path $work 'classes.jar'
Run-Tool (Join-Path $JavaBin 'jar.exe') @('--create','--file',$classJar,'-C',$classes,'.')
Run-Tool (Join-Path $JavaBin 'java.exe') (@('-Xmx384m','-cp',(Join-Path $tools.FullName 'lib/d8.jar'),'com.android.tools.r8.D8','--min-api',$minimumSdk,'--lib',$androidJar,'--output',$dex,$classJar,$pairingJar) + $runtime)
$unsigned = Join-Path $work 'unsigned.apk'
Run-Tool (Join-Path $tools.FullName 'aapt2.exe') @('link','-I',$androidJar,'--manifest',$ManifestPath,'--min-sdk-version',$minimumSdk,'--target-sdk-version','35','-o',$unsigned)
Add-Type -AssemblyName System.IO.Compression.FileSystem
$zip = [System.IO.Compression.ZipFile]::Open($unsigned, [System.IO.Compression.ZipArchiveMode]::Update)
try { [System.IO.Compression.ZipFileExtensions]::CreateEntryFromFile($zip, (Join-Path $dex 'classes.dex'), 'classes.dex') | Out-Null
    foreach($license in @('LICENSE','NOTICE')) {
        [System.IO.Compression.ZipFileExtensions]::CreateEntryFromFile($zip, (Join-Path $pairingRoot ('vendor/google-ukey2/'+$license)), ('assets/licenses/google-ukey2-'+$license)) | Out-Null
    } } finally { $zip.Dispose() }
$aligned = Join-Path $work 'aligned.apk'
Run-Tool (Join-Path $tools.FullName 'zipalign.exe') @('-f','4',$unsigned,$aligned)
$apk = Join-Path $work 'teamy-broker-DUMMY-ONLY-debug.apk'
# Public Makepad debug identity only. Never reuse this for real-token deployment.
$storePassword = 'pass:android'
$keyPassword = 'pass:android'
if ($SigningPasswordEnvironment) {
    if ($SigningPasswordEnvironment -notmatch '^[A-Za-z_][A-Za-z0-9_]*$' -or $KeyPasswordEnvironment -notmatch '^[A-Za-z_][A-Za-z0-9_]*$') { throw 'Password environment variable names are required.' }
    $storePassword = 'env:' + $SigningPasswordEnvironment
    $keyPassword = 'env:' + $KeyPasswordEnvironment
}
Run-SigningTool (Join-Path $JavaBin 'java.exe') @('-jar',(Join-Path $tools.FullName 'lib/apksigner.jar'),'sign','--ks',$key,'--ks-key-alias',$SigningAlias,'--ks-pass',$storePassword,'--key-pass',$keyPassword,'--out',$apk,$aligned)
if ($ExpectedSignerSha256) {
    $certificate = & (Join-Path $JavaBin 'java.exe') '-jar' (Join-Path $tools.FullName 'lib/apksigner.jar') 'verify' '--print-certs' $apk
    if ($LASTEXITCODE) { throw 'Release certificate verification failed.' }
    $match = $certificate | Select-String 'Signer #1 certificate SHA-256 digest: ([0-9a-fA-F]{64})'
    if (!$match -or $match.Count -ne 1 -or $match.Matches.Groups[1].Value.ToLowerInvariant() -ne $ExpectedSignerSha256.ToLowerInvariant()) { throw 'Release signer does not match the supplied public fingerprint.' }
}
Run-Tool (Join-Path $JavaBin 'java.exe') @('-jar',(Join-Path $tools.FullName 'lib/apksigner.jar'),'verify','--verbose',$apk)
Run-Tool (Join-Path $tools.FullName 'aapt2.exe') @('dump','permissions',$apk)
$published = Join-Path $out 'teamy-broker-DUMMY-ONLY-debug.apk'
Move-Item -LiteralPath $apk -Destination $published -Force
$apk = $published
Get-FileHash -LiteralPath $apk -Algorithm SHA256
Write-Output "Dummy-only debug APK: $apk"
} finally {
    # Delete only this invocation's generated workspace, never prior output or caller files.
    $outPath=[IO.Path]::GetFullPath($out).TrimEnd([IO.Path]::DirectorySeparatorChar)+[IO.Path]::DirectorySeparatorChar
    $workPath=[IO.Path]::GetFullPath($work)
    if(!$workPath.StartsWith($outPath,[StringComparison]::OrdinalIgnoreCase)){throw 'Build workspace escaped output directory.'}
    if(Test-Path -LiteralPath $workPath){Remove-Item -LiteralPath $workPath -Recurse -Force}
}

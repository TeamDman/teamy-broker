param(
    [Parameter(Mandatory)][string]$SdkRoot,
    [Parameter(Mandatory)][string]$Keystore,
    [Parameter(Mandatory)][string]$SigningAlias,
    [Parameter(Mandatory)][string]$SigningPasswordEnvironment,
    [Parameter(Mandatory)][string]$KeyPasswordEnvironment,
    [Parameter(Mandatory)][string]$ExpectedSignerSha256,
    [string]$JavaBin = (Split-Path (Get-Command javac -ErrorAction Stop).Source)
)
$ErrorActionPreference = 'Stop'
# Uses an explicitly supplied existing key. Never generates keys or installs/pairs.
$publicDebug = 'e96d983997281700b9d0ecc674ec11edec96d433c0fa9ddd5c216479f2b738c9'
if ($ExpectedSignerSha256 -notmatch '^[0-9a-fA-F]{64}$' -or $ExpectedSignerSha256.ToLowerInvariant() -eq $publicDebug) { throw 'An approved private production signer fingerprint is required.' }
if ($SigningPasswordEnvironment -notmatch '^[A-Za-z_][A-Za-z0-9_]*$' -or $KeyPasswordEnvironment -notmatch '^[A-Za-z_][A-Za-z0-9_]*$') { throw 'Password environment variable names are required.' }
$out = Join-Path $PSScriptRoot 'build/release'
New-Item -ItemType Directory -Force -Path $out | Out-Null
$manifest = Join-Path $out 'AndroidManifest.xml'
$content = [IO.File]::ReadAllText((Join-Path $PSScriptRoot 'AndroidManifest.xml'))
$content = $content.Replace('package="dev.teamy.broker.dummy"','package="dev.teamy.broker.release"').Replace('android:debuggable="true"','android:debuggable="false"').Replace('android:name=".PhoneService"','android:name="dev.teamy.broker.dummy.PhoneService"').Replace('android:name=".BrokerActivity"','android:name="dev.teamy.broker.dummy.BrokerActivity"').Replace('Teamy Broker · NO VAULT','Teamy Broker').Replace('android:versionCode="19"','android:versionCode="24"').Replace('android:versionName="0.19"','android:versionName="0.24"')
[xml]$generated = $content
$ns = 'http://schemas.android.com/apk/res/android'
$app = $generated.manifest.application
if($generated.manifest.package -ne 'dev.teamy.broker.release' -or
   $generated.manifest.GetAttribute('versionCode',$ns) -ne '24' -or
   $generated.manifest.GetAttribute('versionName',$ns) -ne '0.24' -or
   $app.GetAttribute('debuggable',$ns) -ne 'false' -or
   $app.GetAttribute('allowBackup',$ns) -ne 'false' -or
   $app.GetAttribute('fullBackupContent',$ns) -ne 'false') { throw 'Generated release identity/security manifest rejected before signing.' }
[IO.File]::WriteAllText($manifest,$content,[Text.UTF8Encoding]::new($false))
& (Join-Path $PSScriptRoot 'build-debug.ps1') -SdkRoot $SdkRoot -DebugKeystore $Keystore -JavaBin $JavaBin -ManifestPath $manifest -OutputDirectory $out -SigningAlias $SigningAlias -SigningPasswordEnvironment $SigningPasswordEnvironment -KeyPasswordEnvironment $KeyPasswordEnvironment -ExpectedSignerSha256 $ExpectedSignerSha256
if ($LASTEXITCODE) { throw 'Release build failed.' }
$source = Join-Path $out 'teamy-broker-DUMMY-ONLY-debug.apk'
$target = Join-Path $out 'teamy-broker-release.apk'
Move-Item -LiteralPath $source -Destination $target -Force
Get-FileHash -LiteralPath $target -Algorithm SHA256
Write-Output "Prepared release APK: $target"

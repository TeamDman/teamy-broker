# Dot-source this script to load a local profile into the current process only.
param([string]$Profile = (Join-Path $PSScriptRoot 'private-phone-profile.local.json'))
$ErrorActionPreference = 'Stop'
$names = @('TEAMY_BROKER_DESKTOP_IP','TEAMY_BROKER_PHONE_IP','TEAMY_BROKER_DESKTOP_NODE','TEAMY_BROKER_PHONE_NODE','TEAMY_BROKER_TAILNET_USER','TEAMY_BROKER_PHONE_NAME','TEAMY_BROKER_JAVA')
$data = Get-Content -LiteralPath $Profile -Raw | ConvertFrom-Json
foreach ($name in $names) {
    $value = $data.$name
    if ($value -isnot [string] -or [string]::IsNullOrWhiteSpace($value)) { throw 'Private phone profile is incomplete.' }
}
foreach ($name in $names) {
    [Environment]::SetEnvironmentVariable($name, $data.$name, 'Process')
}
Write-Output 'Private phone profile loaded into this process.'

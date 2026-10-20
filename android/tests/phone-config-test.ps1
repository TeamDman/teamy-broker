$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot '../phone-config.ps1')
$names = @('TEAMY_BROKER_DESKTOP_IP','TEAMY_BROKER_PHONE_IP','TEAMY_BROKER_DESKTOP_NODE','TEAMY_BROKER_PHONE_NODE','TEAMY_BROKER_TAILNET_USER','TEAMY_BROKER_PHONE_NAME')
$saved = @{}
foreach ($name in $names) { $saved[$name] = [Environment]::GetEnvironmentVariable($name,'Process'); [Environment]::SetEnvironmentVariable($name,$null,'Process') }
$out = Join-Path $PSScriptRoot '../build/config-test'
function Compile-Config([bool]$Expected) {
    $config = Write-PhoneConfig $out
    & javac -d $out $config (Join-Path $PSScriptRoot 'PrivatePhoneConfigTest.java')
    if ($LASTEXITCODE) { throw 'Configuration compile failed.' }
    & java -cp $out dev.teamy.broker.dummy.PrivatePhoneConfigTest $Expected.ToString().ToLowerInvariant()
    if ($LASTEXITCODE) { throw 'Configuration state test failed.' }
}
try {
    Compile-Config $false
    [Environment]::SetEnvironmentVariable($names[0],(@('100','64','0','1') -join '.'),'Process')
    try { Write-PhoneConfig $out | Out-Null; throw 'Incomplete profile accepted.' } catch { if ($_.Exception.Message -eq 'Incomplete profile accepted.') { throw } }
    [Environment]::SetEnvironmentVariable($names[1],(@('100','64','0','2') -join '.'),'Process')
    [Environment]::SetEnvironmentVariable($names[2],'desktop-test','Process')
    [Environment]::SetEnvironmentVariable($names[3],'phone-test','Process')
    [Environment]::SetEnvironmentVariable($names[4],'42','Process')
    [Environment]::SetEnvironmentVariable($names[5],'phone.example.invalid.','Process')
    Compile-Config $true
    [Environment]::SetEnvironmentVariable($names[2],'node"; invalid','Process')
    try { Write-PhoneConfig $out | Out-Null; throw 'Invalid identity accepted.' } catch { if ($_.Exception.Message -eq 'Invalid identity accepted.') { throw } }
    Write-Output 'Absent, complete, partial and invalid phone profiles checked.'
} finally {
    foreach ($name in $names) { [Environment]::SetEnvironmentVariable($name,$saved[$name],'Process') }
}

# Pure policy fixtures: no network, subprocesses or file mutations.
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot '../../tools/publication-policy.ps1')
$address = '100' + '.64.0.1'
$otherAddress = '100' + '.64.0.2'
$allowlist = @([pscustomobject]@{
    file = 'tests/fixture.rs'
    rule = 'Tailnet address or hostname'
    matches = @($address)
    reason = 'Synthetic policy fixture.'
})
Assert-PublicationAllowlist $allowlist
if (@(Get-PublicationFindings 'tests/fixture.rs' $address $allowlist).Count) { throw 'Exact fixture exception was rejected.' }
if (@(Get-PublicationFindings 'src/fixture.rs' $address $allowlist).Count -ne 1) { throw 'Exception leaked into another file.' }
if (@(Get-PublicationFindings 'tests/fixture.rs' "$address $otherAddress" $allowlist).Count -ne 1) { throw 'Exception hid a different value in the same file.' }
$token = 'ghp_' + ('a' * 36)
$findings = @(Get-PublicationFindings 'tests/fixture.rs' "$address $token" $allowlist)
if ($findings.Count -ne 1 -or $findings[0].rule -ne 'GitHub token') { throw 'Fixture exception suppressed an unrelated secret rule.' }
if (@(Get-PublicationFindings 'tests/fixture.rs' ($address + '0') $allowlist).Count -ne 1) { throw 'Exception used partial matching.' }
$constructor = 'Ipv4Addr::new(' + '100, 64, 0, 1)'
if (@(Get-PublicationFindings 'tests/fixture.rs' $constructor $allowlist).Count -ne 1) { throw 'Constructor representation was not scanned independently.' }
$badAllowlist = @([pscustomobject]@{
    file = 'tests/fixture.rs'
    rule = 'Tailnet address or hostname'
    matches = @('.*')
    reason = 'This wildcard must not become an exception.'
})
$rejected = $false
try { Assert-PublicationAllowlist $badAllowlist } catch { $rejected = $true }
if (-not $rejected) { throw 'Wildcard policy exception was accepted.' }
Write-Output 'Publication policy fixtures passed: exact file/rule/value exceptions preserve all other findings.'

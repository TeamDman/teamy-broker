#requires -Version 7.0
param([switch]$ActivateDummyTailnet, [int]$DurationSeconds = 180)
$ErrorActionPreference = 'Stop'
# This script's default is a read-only proposal. Never use Serve/Funnel/firewall changes.
# Activation must be separately approved; no persistent identities, certificates or state are written.
$desktopIp = [Net.IPAddress]::Parse($env:TEAMY_BROKER_DESKTOP_IP)
$phoneIp = [Net.IPAddress]::Parse($env:TEAMY_BROKER_PHONE_IP)
$desktopId = $env:TEAMY_BROKER_DESKTOP_NODE
$phoneId = $env:TEAMY_BROKER_PHONE_NODE
$userId = [UInt64]::Parse($env:TEAMY_BROKER_TAILNET_USER)
$phoneName = $env:TEAMY_BROKER_PHONE_NAME
if (!$desktopId -or !$phoneId -or !$phoneName -or !$userId) { throw 'Private phone configuration required.' }
$port = 43188
$tailscale = 'C:\Program Files\Tailscale\tailscale.exe'
$demo = Join-Path (Split-Path $PSScriptRoot) 'target/debug/examples/mobile_handoff_tailnet.exe'
if ($DurationSeconds -lt 1 -or $DurationSeconds -gt 300) { throw 'Dummy test duration must be 1..300 seconds.' }
function Read-TailscaleJson([string[]]$Arguments) {
    $start = [Diagnostics.ProcessStartInfo]::new($tailscale)
    $start.UseShellExecute = $false; $start.CreateNoWindow = $true
    $start.RedirectStandardOutput = $true; $start.RedirectStandardError = $true
    foreach ($argument in $Arguments) { $start.ArgumentList.Add($argument) }
    $process = [Diagnostics.Process]::new(); $process.StartInfo = $start
    try {
        $wait = if ($script:until) { [Math]::Min(3000,[Math]::Max(1,($script:until - [DateTimeOffset]::UtcNow).TotalMilliseconds)) } else {3000}
        if (!$process.Start()) { throw 'Tailscale inspection unavailable.' }
        $stdout=$process.StandardOutput.ReadToEndAsync(); $stderr=$process.StandardError.ReadToEndAsync()
        if (!$process.WaitForExit([int]$wait)) { if (!$process.HasExited) { $process.Kill() }; throw 'Tailscale inspection unavailable.' }
        if ($process.ExitCode -ne 0) { throw 'Tailscale inspection unavailable.' }
        return $stdout.GetAwaiter().GetResult() | ConvertFrom-Json
    } finally { $process.Dispose() }
}
function Assert-DesktopState {
    $state = Read-TailscaleJson @('status','--json')
    if ($state.BackendState -cne 'Running' -or $state.Self.ID -cne $desktopId -or $state.Self.OS -cne 'windows' -or $state.Self.UserID -ne $userId -or $state.Self.TailscaleIPs -notcontains $desktopIp.ToString()) { throw 'Inspected desktop identity changed; do not bind.' }
}
function Assert-PhoneState {
    $state = Read-TailscaleJson @('whois','--json',$phoneIp.ToString())
    if ($state.Node.StableID -cne $phoneId -or $state.Node.User -ne $userId -or $state.Node.Hostinfo.OS -cne 'android' -or $state.Node.Name -cne $phoneName -or $state.Node.Addresses -notcontains "$phoneIp/32" -or [DateTimeOffset]::Parse($state.Node.KeyExpiry) -le [DateTimeOffset]::UtcNow) { throw 'Inspected phone identity changed; do not accept.' }
}
Assert-DesktopState
Assert-PhoneState
$proposal = [pscustomobject]@{ Bind="$desktopIp`:$port"; AllowedPhone="$phoneIp/32"; DesktopNode=$desktopId; PhoneNode=$phoneId; DurationSeconds=$DurationSeconds; Auth='TLS 1.2+, exact temporary certificate pin, existing encrypted session protocol'; ProductionMode=$false; Activated=$false }
if (!$ActivateDummyTailnet) { $proposal | Format-List; return }

if (!(Test-Path -LiteralPath $demo)) { throw 'Build the reviewed Rust TLS example first; no network changes were made.' }
# This exact scope is user-approved. Wait for explicit phone readiness before running this switch.
# The native bootstrap appears before binding; only its OK begins the listener/request window.
# Its current scope is a fixed 180-second session; other durations require a reviewed source change.
if ($DurationSeconds -ne 180) { throw 'This reviewed Rust example uses exactly a 180-second limit.' }
& $demo --activate-dummy-tailnet
if ($LASTEXITCODE) { throw 'Dummy tailnet test unavailable or activation remains blocked.' }

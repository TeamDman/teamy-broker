#requires -Version 7.0
[CmdletBinding()]
param([string]$TargetDirectory)
$ErrorActionPreference='Stop'
# This tests the host TLS fixture under Java17 against Rustls on isolated loopback only.
# It never calls the tailnet activation entry point, starts a desktop dialog, or writes private keys.
if (-not $TargetDirectory) {
    $manifest = Join-Path (Split-Path $PSScriptRoot) 'Cargo.toml'
    $metadataText = & cargo metadata --offline --locked --no-deps --format-version 1 --manifest-path $manifest
    if ($LASTEXITCODE -ne 0) { throw 'Cargo metadata failed before locating the TLS probe.' }
    $TargetDirectory = ($metadataText | ConvertFrom-Json).target_directory
}
$probe=Join-Path $TargetDirectory 'debug/examples/mobile_tls_probe.exe'
if (!(Test-Path -LiteralPath $probe)) { throw 'Build mobile_tls_probe in a serialized slot first.' }
$out=Join-Path $PSScriptRoot 'build/tls-check'; New-Item -ItemType Directory -Path $out -Force | Out-Null
$sources=@('tests/support/dev/teamy/broker/dummy/PinnedTls.java','tests/TlsTest.java') | ForEach-Object {Join-Path $PSScriptRoot $_}
& javac -encoding UTF-8 -d $out @sources
if ($LASTEXITCODE) { throw 'TLS test source compilation failed.' }
foreach ($wrong in @($false,$true)) {
    $server=$null; $client=$null
    try {
        $start=[Diagnostics.ProcessStartInfo]::new($probe);$start.UseShellExecute=$false;$start.CreateNoWindow=$true
        $start.RedirectStandardOutput=$true;$start.RedirectStandardError=$true
        $server=[Diagnostics.Process]::new();$server.StartInfo=$start;[void]$server.Start()
        $line=$server.StandardOutput.ReadLineAsync().WaitAsync([TimeSpan]::FromSeconds(3)).GetAwaiter().GetResult()
        $serverError=$server.StandardError.ReadToEndAsync()
        if ($line -notmatch '^(\d{1,5}) ([0-9a-f]{64})$') { throw 'Isolated probe bootstrap rejected.' }
        $port=$Matches[1];$pin=$Matches[2]
        $start=[Diagnostics.ProcessStartInfo]::new((Get-Command java).Source);$start.UseShellExecute=$false;$start.CreateNoWindow=$true
        $start.RedirectStandardOutput=$true;$start.RedirectStandardError=$true
        foreach($arg in @('-cp',$out,'dev.teamy.broker.dummy.TlsTest',$port,$(if($wrong){'0'*64}else{$pin}))) {$start.ArgumentList.Add($arg)}
        if($wrong){$start.ArgumentList.Add('reject')}
        $client=[Diagnostics.Process]::new();$client.StartInfo=$start;[void]$client.Start()
        $clientOut=$client.StandardOutput.ReadToEndAsync();$clientError=$client.StandardError.ReadToEndAsync()
        if(!$client.WaitForExit(5000) -or $client.ExitCode -ne 0) {throw 'Isolated Java TLS pin test failed.'}
        if(!$server.WaitForExit(5000) -or $server.ExitCode -ne 0) {throw 'Isolated Rust TLS probe failed.'}
        Write-Output $(if($wrong){'Host TLS fixture rejected a wrong certificate pin.'}else{'Host TLS fixture completed TLS1.2 and a fixed-dummy frame exchange with Rustls.'})
    } finally {
        if($client){if(!$client.HasExited){$client.Kill()};$client.Dispose()}
        if($server){if(!$server.HasExited){$server.Kill()};$server.Dispose()}
    }
}
Write-Output 'Local JVM/Rust TLS tests complete; no tailnet listener or persistent TLS key was created.'

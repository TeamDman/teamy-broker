#requires -Version 7.0
$ErrorActionPreference='Stop'
# This tests the exact Android TLS helper under Java17 against Rustls on isolated loopback only.
# It never calls the tailnet activation entry point, starts a desktop dialog, or writes private keys.
$probe=Join-Path (Split-Path $PSScriptRoot) 'target/debug/examples/mobile_tls_probe.exe'
if (!(Test-Path -LiteralPath $probe)) { throw 'Build mobile_tls_probe in a serialized slot first.' }
$out=Join-Path $PSScriptRoot 'build/tls-check'; New-Item -ItemType Directory -Path $out -Force | Out-Null
$sources=@('src/dev/teamy/broker/dummy/Crypto.java','src/dev/teamy/broker/dummy/DummyPolicy.java','src/dev/teamy/broker/dummy/PinnedTls.java','tests/TlsTest.java') | ForEach-Object {Join-Path $PSScriptRoot $_}
. (Join-Path $PSScriptRoot 'phone-config.ps1')
$sources += Write-PhoneConfig (Join-Path $out 'generated')
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
        Write-Output $(if($wrong){'Exact Android TLS helper rejected a wrong certificate pin.'}else{'Exact Android TLS helper completed TLS1.2 and a fixed-dummy frame exchange with Rustls.'})
    } finally {
        if($client){if(!$client.HasExited){$client.Kill()};$client.Dispose()}
        if($server){if(!$server.HasExited){$server.Kill()};$server.Dispose()}
    }
}
Write-Output 'Local JVM/Rust TLS tests complete; no tailnet listener or persistent TLS key was created.'

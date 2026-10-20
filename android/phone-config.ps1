function Write-PhoneConfig([string]$OutputDirectory) {
    $names = @('TEAMY_BROKER_DESKTOP_IP','TEAMY_BROKER_PHONE_IP','TEAMY_BROKER_DESKTOP_NODE','TEAMY_BROKER_PHONE_NODE','TEAMY_BROKER_TAILNET_USER','TEAMY_BROKER_PHONE_NAME')
    $values = @($names | ForEach-Object { [Environment]::GetEnvironmentVariable($_, 'Process') })
    $present = @($values | Where-Object { $_ }).Count
    if ($present -ne 0 -and $present -ne $names.Count) { throw 'Incomplete private phone configuration.' }
    $configured = $present -eq $names.Count
    if (!$configured) { $values = @('192.0.2.10','192.0.2.11','desktop-placeholder','phone-placeholder','0','phone.invalid.') }
    foreach ($index in @(0,1)) {
        $ip = $null
        if (![Net.IPAddress]::TryParse($values[$index], [ref]$ip) -or $ip.AddressFamily -ne [Net.Sockets.AddressFamily]::InterNetwork) { throw 'Invalid phone address.' }
        $octets = $ip.GetAddressBytes()
        if ($configured -and ($octets[0] -ne 100 -or $octets[1] -lt 64 -or $octets[1] -gt 127)) { throw 'Phone addresses must be Tailscale IPv4 addresses.' }
        $values[$index] = $ip.ToString()
    }
    if ($values[0] -eq $values[1]) { throw 'Phone addresses must differ.' }
    foreach ($index in @(2,3,5)) {
        if ($values[$index] -notmatch '^[A-Za-z0-9._-]{1,255}$') { throw 'Invalid phone identity.' }
    }
    $user = 0UL
    if (![UInt64]::TryParse($values[4], [ref]$user) -or ($configured -and !$user)) { throw 'Invalid tailnet user.' }
    $literal = if ($configured) { 'true' } else { 'false' }
    $source = @"
package dev.teamy.broker.dummy;
final class PrivatePhoneConfig {
    static final boolean CONFIGURED = $literal;
    static final String DESKTOP = "$($values[0])";
    static final String PHONE = "$($values[1])";
    static final String DESKTOP_NODE = "$($values[2])";
    static final String PHONE_NODE = "$($values[3])";
    static final int PORT = 43188;
    private PrivatePhoneConfig() {}
    static void requireConfigured() throws Exception {
        if (!CONFIGURED) throw new Exception("private phone configuration unavailable");
    }
}
"@
    New-Item -ItemType Directory -Path $OutputDirectory -Force | Out-Null
    $path = Join-Path $OutputDirectory 'PrivatePhoneConfig.java'
    [IO.File]::WriteAllText($path, $source, [Text.UTF8Encoding]::new($false))
    return $path
}

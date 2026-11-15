function Get-PublicationRules {
    [ordered]@{
        'GitHub token' = '(?:gh[pousr]_[A-Za-z0-9]{30,}|github_pat_[A-Za-z0-9_]{50,})'
        'AWS access key' = '(?:AKIA|ASIA)[A-Z0-9]{16}'
        'Private key' = '-----BEGIN (?:RSA |EC |OPENSSH |DSA )?PRIVATE KEY-----'
        '1Password service token' = 'ops_[A-Za-z0-9_-]{30,}'
        'JWT' = 'eyJ[A-Za-z0-9_-]{20,}\.[A-Za-z0-9_-]{20,}\.[A-Za-z0-9_-]{20,}'
        'User local path' = '(?i)(?:[A-Z]:[\\/](?:Users|Programming)[\\/]|/' + 'home/[^/\s]+/)'
        'Tailnet address or hostname' = '(?i)(?:\b100\.(?:6[4-9]|[7-9][0-9]|1[01][0-9]|12[0-7])\.\d{1,3}\.\d{1,3}\b|Ipv4Addr::new\(100,\s*(?:6[4-9]|[7-9][0-9]|1[01][0-9]|12[0-7]),\s*\d{1,3},\s*\d{1,3}\)|\b[a-z0-9-]+\.ts\.net\b)'
        'Non-synthetic item reference' = 'op://(?!(?:Synthetic|YourVault)/|Fixture/Approval/input(?=["\s]|$))[A-Za-z0-9][^\s"`]+/'
    }
}

function Assert-PublicationAllowlist {
    param([object[]]$Allowlist)
    $rules = Get-PublicationRules
    foreach ($entry in $Allowlist) {
        if (-not $entry.file -or [IO.Path]::IsPathRooted($entry.file) -or
            $entry.file -match '(^|/)\.\.(/|$)|\\' -or
            -not $rules.Contains($entry.rule) -or -not $entry.reason -or
            @($entry.matches).Count -eq 0) {
            throw 'Publication allowlist entries require a relative file, known rule, exact matches and a reason.'
        }
        foreach ($value in $entry.matches) {
            $matches = [regex]::Matches($value, $rules[$entry.rule])
            if ($matches.Count -ne 1 -or $matches[0].Value -cne $value) {
                throw 'Publication allowlist values must equal one complete rule match.'
            }
        }
    }
}

function Get-PublicationFindings {
    param([string]$RelativePath, [string]$Text, [object[]]$Allowlist = @())
    $path = $RelativePath.Replace('\', '/')
    foreach ($rule in (Get-PublicationRules).GetEnumerator()) {
        $permitted = @($Allowlist | Where-Object { $_.file -ceq $path -and $_.rule -ceq $rule.Key } | ForEach-Object { $_.matches })
        foreach ($match in [regex]::Matches($Text, $rule.Value)) {
            if ($match.Value -cnotin $permitted) {
                [pscustomobject]@{ file = $path; rule = $rule.Key }
                break
            }
        }
    }
}

[CmdletBinding()]
param([string]$Root=$PSScriptRoot, [switch]$TrackedOnly)
$ErrorActionPreference='Stop'
$rules=@{
 'GitHub token'='(?:gh[pousr]_[A-Za-z0-9]{30,}|github_pat_[A-Za-z0-9_]{50,})'
 'AWS access key'='(?:AKIA|ASIA)[A-Z0-9]{16}'
 'Private key'='-----BEGIN (?:RSA |EC |OPENSSH |DSA )?PRIVATE KEY-----'
 '1Password service token'='ops_[A-Za-z0-9_-]{30,}'
 'JWT'='eyJ[A-Za-z0-9_-]{20,}\.[A-Za-z0-9_-]{20,}\.[A-Za-z0-9_-]{20,}'
 'User local path'='(?i)(?:[A-Z]:[\\/](?:Users|Programming)[\\/]|/'+'home/[^/\s]+/)'
 'Tailnet address or hostname'='(?i)(?:\b100\.(?:6[4-9]|[7-9][0-9]|1[01][0-9]|12[0-7])\.\d{1,3}\.\d{1,3}\b|Ipv4Addr::new\(100,\s*(?:6[4-9]|[7-9][0-9]|1[01][0-9]|12[0-7]),|\b[a-z0-9-]+\.ts\.net\b)'
 'Non-synthetic item reference'='op://(?!(?:Synthetic|YourVault)/|Fixture/Approval/input(?=["\s]|$))[A-Za-z0-9][^\s"`]+/'
}
if ($TrackedOnly) {
 $files = @(& git -C $Root ls-files --cached | ForEach-Object { Join-Path $Root $_ })
 if ($LASTEXITCODE) { throw 'Tracked source inventory failed' }
} else {
 $files=& rg --files --hidden -g '!.git/**' -g '!target/**' -g '!tracy/**' $Root
 if($LASTEXITCODE -gt 1){throw 'Source inventory failed'}
}
$findings=@()
foreach($file in $files){
 if([IO.Path]::GetExtension($file) -notin @('.rs','.java','.proto','.toml','.md','.ps1','.json','.yml','.yaml','.xml','.html','.txt','.rc','.manifest','.lock','.gitignore') -and [IO.Path]::GetFileName($file) -ne 'LICENSE'){continue}
 $text=[IO.File]::ReadAllText($file)
 foreach($rule in $rules.GetEnumerator()){if([regex]::IsMatch($text,$rule.Value)){$findings+=@{file=[IO.Path]::GetRelativePath($Root,$file);rule=$rule.Key}}}
}
if($findings.Count){$findings|ForEach-Object {Write-Output ($_.file+': '+$_.rule)};throw 'Publication scan has findings; values suppressed'}
foreach($path in @('Cargo.toml','sdk/Cargo.toml')){if([IO.File]::ReadAllText((Join-Path $Root $path)) -notmatch 'license\s*=\s*"MPL-2.0"'){throw 'License metadata mismatch'}}
if([IO.File]::ReadAllText((Join-Path $Root 'LICENSE')) -notmatch 'Mozilla Public License Version 2.0'){throw 'MPL license text missing'}
Write-Output ('Publication scan passed: '+$files.Count+' source files; '+$rules.Count+' secret/private-data rules; MPL-2.0 metadata and license verified. This pattern scan is not a guarantee against every secret format.')

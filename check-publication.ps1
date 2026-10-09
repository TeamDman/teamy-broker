[CmdletBinding()]
param([string]$Root=$PSScriptRoot)
$ErrorActionPreference='Stop'
$rules=@{
 'GitHub token'='(?:gh[pousr]_[A-Za-z0-9]{30,}|github_pat_[A-Za-z0-9_]{50,})'
 'AWS access key'='(?:AKIA|ASIA)[A-Z0-9]{16}'
 'Private key'='-----BEGIN (?:RSA |EC |OPENSSH |DSA )?PRIVATE KEY-----'
 '1Password service token'='ops_[A-Za-z0-9_-]{30,}'
 'JWT'='eyJ[A-Za-z0-9_-]{20,}\.[A-Za-z0-9_-]{20,}\.[A-Za-z0-9_-]{20,}'
 'User local path'='(?i)(?:[A-Z]:[\/](?:Users|Programming)[\/]|/'+'home/[^/\s]+/)'
 'Non-synthetic item reference'='op://(?!(?:Synthetic|YourVault)/)[A-Za-z0-9][^\s"`]+/'
}
$files=& rg --files --hidden -g '!.git/**' -g '!target/**' -g '!tracy/**' $Root
if($LASTEXITCODE -gt 1){throw 'Source inventory failed'}
$findings=@()
foreach($file in $files){
 if([IO.Path]::GetExtension($file) -notin @('.rs','.toml','.md','.ps1','.json','.yml','.yaml','.rc','.manifest','.lock','.gitignore') -and [IO.Path]::GetFileName($file) -ne 'LICENSE'){continue}
 $text=[IO.File]::ReadAllText($file)
 foreach($rule in $rules.GetEnumerator()){if([regex]::IsMatch($text,$rule.Value)){$findings+=@{file=[IO.Path]::GetRelativePath($Root,$file);rule=$rule.Key}}}
}
if($findings.Count){$findings|ForEach-Object {Write-Output ($_.file+': '+$_.rule)};throw 'Publication scan has findings; values suppressed'}
foreach($path in @('Cargo.toml','sdk/Cargo.toml')){if([IO.File]::ReadAllText((Join-Path $Root $path)) -notmatch 'license\s*=\s*"MPL-2.0"'){throw 'License metadata mismatch'}}
if([IO.File]::ReadAllText((Join-Path $Root 'LICENSE')) -notmatch 'Mozilla Public License Version 2.0'){throw 'MPL license text missing'}
Write-Output ('Publication scan passed: '+$files.Count+' source files; 7 secret/private-data rules; MPL-2.0 metadata and license verified. This pattern scan is not a guarantee against every secret format.')

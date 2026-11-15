#requires -Version 7.0
[CmdletBinding()]
param([string]$Root = $PSScriptRoot, [switch]$TrackedOnly)
$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $false
$Root = (Resolve-Path -LiteralPath $Root).Path
. (Join-Path $PSScriptRoot 'tools/publication-policy.ps1')
$allowlist = @(Get-Content -Raw -LiteralPath (Join-Path $PSScriptRoot 'tools/publication-allowlist.json') | ConvertFrom-Json)
Assert-PublicationAllowlist $allowlist

if ($TrackedOnly) {
    $tracked = & git -C $Root ls-files --cached -z
    if ($LASTEXITCODE -ne 0) { throw 'Tracked source inventory failed.' }
    $files = @(($tracked -join "`n").Split([char]0, [StringSplitOptions]::RemoveEmptyEntries) | ForEach-Object { Join-Path $Root $_ })
} else {
    $files = @(& rg --files --hidden -g '!.git/**' -g '!target/**' -g '!tracy/**' $Root)
    if ($LASTEXITCODE -gt 1) { throw 'Source inventory failed.' }
}
$findings = @()
$scanned = 0
$manifests = @()
$extensions = @('.rs', '.java', '.proto', '.toml', '.md', '.ps1', '.psm1', '.psd1', '.json', '.yml', '.yaml', '.xml', '.html', '.txt', '.rc', '.manifest', '.lock', '.gitignore', '.properties', '.gradle', '.sh', '.bat', '.cmd', '.env')
foreach ($file in $files) {
    # A tracked working-tree deletion has no content to publish. Other read
    # errors still fail the scan, and untracked candidates are included by default.
    if (-not (Test-Path -LiteralPath $file -PathType Leaf)) { continue }
    $name = [IO.Path]::GetFileName($file)
    if ($name -eq 'Cargo.toml') { $manifests += $file }
    if ([IO.Path]::GetExtension($file) -notin $extensions -and $name -notin @('LICENSE', 'NOTICE')) { continue }
    $text = [IO.File]::ReadAllText($file)
    $findings += @(Get-PublicationFindings ([IO.Path]::GetRelativePath($Root, $file)) $text $allowlist)
    $scanned++
}
if ($findings.Count) {
    $findings | ForEach-Object { Write-Output ($_.file + ': ' + $_.rule) }
    throw 'Publication scan has findings; values suppressed.'
}
foreach ($manifest in $manifests) {
    if ([IO.File]::ReadAllText($manifest) -notmatch 'license\s*=\s*"MPL-2.0"') { throw 'First-party package license metadata mismatch.' }
}
if ($manifests.Count -eq 0) { throw 'No first-party package manifests were scanned.' }
if ([IO.File]::ReadAllText((Join-Path $Root 'LICENSE')) -notmatch 'Mozilla Public License Version 2.0') { throw 'MPL license text missing.' }
Write-Output "Publication scan passed: $scanned text files; $((Get-PublicationRules).Count) rules; $($manifests.Count) package licenses. Exact fixture exceptions are reviewed in tools/publication-allowlist.json. Pattern checks do not detect every secret format."

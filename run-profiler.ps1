param(
	[Parameter(ValueFromRemainingArguments = $true)]
	[string[]]$QueryArgs
)

if (-not $QueryArgs -or $QueryArgs.Count -eq 0) {
	throw 'Supply the broker command to profile, for example: ./run-profiler.ps1 preview'
}

$profiler = Get-Command teamy-profiler -ErrorAction SilentlyContinue
if (-not $profiler) {
	throw "teamy-profiler not found in PATH"
}

& $profiler.Source run cargo `
	--project $PSScriptRoot `
	--bin teamy-broker `
	--profile release `
	--feature extended_observability `
	--feature tracy `
	-- @QueryArgs
exit $LASTEXITCODE

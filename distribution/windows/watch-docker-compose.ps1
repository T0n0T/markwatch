param(
  [Parameter(Mandatory = $true)]
  [string]$ComposeDir,

  [string]$EnvFile = "",
  [int]$DebounceMs = 800,
  [int]$ReconcileSec = 600,

  [ValidateSet("error", "warn", "info", "debug")]
  [string]$LogLevel = "info",

  [ValidateSet("cmd", "powershell")]
  [string]$Shell = "powershell",

  [string]$BinaryPath = ""
)

$ErrorActionPreference = "Stop"

if ([string]::IsNullOrWhiteSpace($EnvFile)) {
  $EnvFile = Join-Path $ComposeDir ".env.runtime"
}

if ([string]::IsNullOrWhiteSpace($BinaryPath)) {
  $BinaryPath = Join-Path $PSScriptRoot "mdwatch.exe"
}

if (-not (Test-Path -LiteralPath $ComposeDir -PathType Container)) {
  throw "ComposeDir not found: $ComposeDir"
}
if (-not (Test-Path -LiteralPath $EnvFile -PathType Leaf)) {
  throw "EnvFile not found: $EnvFile"
}
if (-not (Test-Path -LiteralPath $BinaryPath -PathType Leaf)) {
  throw "BinaryPath not found: $BinaryPath"
}

$line = Get-Content -LiteralPath $EnvFile | Where-Object { $_ -match '^MARKDOWN_DIR=' } | Select-Object -First 1
if (-not $line) {
  throw "MARKDOWN_DIR not found in $EnvFile"
}
$markdownDir = $line -replace '^MARKDOWN_DIR=', ''
if ([string]::IsNullOrWhiteSpace($markdownDir)) {
  throw "MARKDOWN_DIR is empty in $EnvFile"
}
if (-not (Test-Path -LiteralPath $markdownDir -PathType Container)) {
  throw "MARKDOWN_DIR not found: $markdownDir"
}

$escapedEnvFile = $EnvFile.Replace("'", "''")
$buildCmd = "docker compose --env-file '$escapedEnvFile' run --rm --no-deps hugo-builder"

& $BinaryPath `
  --root $markdownDir `
  --workdir $ComposeDir `
  --cmd $buildCmd `
  --shell $Shell `
  --debounce-ms $DebounceMs `
  --reconcile-sec $ReconcileSec `
  --log-level $LogLevel

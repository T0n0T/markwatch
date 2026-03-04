param(
  [string]$Version = "",
  [string]$Target = "x86_64-pc-windows-msvc",
  [string]$OutDir = "dist"
)

$ErrorActionPreference = "Stop"

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$projectDir = Resolve-Path (Join-Path $scriptDir "..\..")

if ([string]::IsNullOrWhiteSpace($Version)) {
  $cargoToml = Join-Path $projectDir "Cargo.toml"
  $line = Get-Content -LiteralPath $cargoToml | Where-Object { $_ -match '^version = "' } | Select-Object -First 1
  if (-not $line) {
    throw "Failed to resolve version from Cargo.toml"
  }
  $Version = ($line -replace '^version = "(.*)"$', '$1')
}
$Version = $Version.TrimStart("v")

$binaryPath = Join-Path $projectDir ("target\" + $Target + "\release\mdwatch.exe")
if (-not (Test-Path -LiteralPath $binaryPath -PathType Leaf)) {
  $binaryPath = Join-Path $projectDir "target\release\mdwatch.exe"
}
if (-not (Test-Path -LiteralPath $binaryPath -PathType Leaf)) {
  throw "Release binary not found: $binaryPath"
}

$outDirAbs = Join-Path $projectDir $OutDir
New-Item -ItemType Directory -Force -Path $outDirAbs | Out-Null

$stageName = "mdwatch-$Version-$Target"
$stageDir = Join-Path $outDirAbs $stageName
$zipPath = Join-Path $outDirAbs ("$stageName.zip")

if (Test-Path -LiteralPath $stageDir) {
  Remove-Item -Recurse -Force -LiteralPath $stageDir
}
if (Test-Path -LiteralPath $zipPath) {
  Remove-Item -Force -LiteralPath $zipPath
}

New-Item -ItemType Directory -Force -Path $stageDir | Out-Null
Copy-Item -LiteralPath $binaryPath -Destination (Join-Path $stageDir "mdwatch.exe")
Copy-Item -LiteralPath (Join-Path $projectDir "README.md") -Destination (Join-Path $stageDir "README.md")
Copy-Item -LiteralPath (Join-Path $projectDir "distribution\windows\watch-docker-compose.ps1") -Destination (Join-Path $stageDir "watch-docker-compose.ps1")

Compress-Archive -Path (Join-Path $stageDir "*") -DestinationPath $zipPath -Force
Write-Host "Created $zipPath"

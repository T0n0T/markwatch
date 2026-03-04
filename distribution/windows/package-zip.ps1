param(
  [string]$Version = "",
  [string]$Target = "x86_64-pc-windows-msvc",
  [string]$OutDir = "dist"
)

$ErrorActionPreference = "Stop"

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$projectDir = Resolve-Path (Join-Path $scriptDir "..\..")

if ([string]::IsNullOrWhiteSpace($Version)) {
  # Keep -Version for backward compatibility, but release asset naming is versionless.
  $Version = ""
}

$binaryPath = Join-Path $projectDir ("target\" + $Target + "\release\markwatch.exe")
if (-not (Test-Path -LiteralPath $binaryPath -PathType Leaf)) {
  $binaryPath = Join-Path $projectDir "target\release\markwatch.exe"
}
if (-not (Test-Path -LiteralPath $binaryPath -PathType Leaf)) {
  throw "Release binary not found: $binaryPath"
}

$outDirAbs = Join-Path $projectDir $OutDir
New-Item -ItemType Directory -Force -Path $outDirAbs | Out-Null

$stageName = "markwatch-$Target"
$stageDir = Join-Path $outDirAbs $stageName
$zipPath = Join-Path $outDirAbs ("$stageName.zip")

if (Test-Path -LiteralPath $stageDir) {
  Remove-Item -Recurse -Force -LiteralPath $stageDir
}
if (Test-Path -LiteralPath $zipPath) {
  Remove-Item -Force -LiteralPath $zipPath
}

New-Item -ItemType Directory -Force -Path $stageDir | Out-Null
Copy-Item -LiteralPath $binaryPath -Destination (Join-Path $stageDir "markwatch.exe")
Copy-Item -LiteralPath (Join-Path $projectDir "README.md") -Destination (Join-Path $stageDir "README.md")
Copy-Item -LiteralPath (Join-Path $projectDir "distribution\windows\watch-markcompose.ps1") -Destination (Join-Path $stageDir "watch-markcompose.ps1")

Compress-Archive -Path (Join-Path $stageDir "*") -DestinationPath $zipPath -Force
Write-Host "Created $zipPath"

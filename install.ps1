#Requires -Version 5
<#
.SYNOPSIS
  diffthing installer for Windows.

.DESCRIPTION
  Downloads the prebuilt diffthing binary from the latest GitHub release,
  verifies its SHA-256 checksum against the release's SHA256SUMS, and installs
  it to %LOCALAPPDATA%\diffthing\bin (or $env:DIFFTHING_INSTALL_DIR).

  Run:
    irm https://diffthing.dev/install.ps1 | iex

  Environment overrides:
    DIFFTHING_VERSION      release tag to install, e.g. v0.3.0 (default: latest)
    DIFFTHING_INSTALL_DIR  install directory
#>

$ErrorActionPreference = 'Stop'

$Repo = 'rahXephonz/diffthing'
$Target = 'x86_64-pc-windows-msvc'
$Archive = "diffthing-$Target.zip"

$version = if ($env:DIFFTHING_VERSION) { $env:DIFFTHING_VERSION } else { 'latest' }
$base = if ($version -eq 'latest') {
  "https://github.com/$Repo/releases/latest/download"
} else {
  "https://github.com/$Repo/releases/download/$version"
}

$tmp = Join-Path $env:TEMP ("diffthing-" + [guid]::NewGuid().ToString())
New-Item -ItemType Directory -Path $tmp | Out-Null
try {
  $archivePath = Join-Path $tmp $Archive
  $sumsPath = Join-Path $tmp 'SHA256SUMS'

  Write-Host "downloading $Archive ($version)"
  Invoke-WebRequest -Uri "$base/$Archive" -OutFile $archivePath -UseBasicParsing
  Invoke-WebRequest -Uri "$base/SHA256SUMS" -OutFile $sumsPath -UseBasicParsing

  Write-Host "verifying checksum"
  $pattern = "\s$([regex]::Escape($Archive))$"
  $expected = Get-Content $sumsPath |
    Where-Object { $_ -match $pattern } |
    ForEach-Object { ($_ -split '\s+')[0] } |
    Select-Object -First 1
  if (-not $expected) { throw "no checksum for $Archive in SHA256SUMS" }
  $actual = (Get-FileHash -Algorithm SHA256 -Path $archivePath).Hash.ToLower()
  if ($expected.ToLower() -ne $actual) {
    throw "checksum mismatch: expected $expected, got $actual"
  }

  Expand-Archive -Path $archivePath -DestinationPath $tmp -Force
  $exeSrc = Join-Path $tmp 'diffthing.exe'
  if (-not (Test-Path $exeSrc)) { throw "archive did not contain diffthing.exe" }

  $installDir = if ($env:DIFFTHING_INSTALL_DIR) {
    $env:DIFFTHING_INSTALL_DIR
  } else {
    Join-Path $env:LOCALAPPDATA 'diffthing\bin'
  }
  New-Item -ItemType Directory -Force -Path $installDir | Out-Null
  Copy-Item -Force $exeSrc (Join-Path $installDir 'diffthing.exe')

  Write-Host "installed diffthing to $installDir\diffthing.exe"

  $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
  if (-not $userPath -or ($userPath -split ';') -notcontains $installDir) {
    Write-Host ""
    Write-Host "note: $installDir is not on your PATH. Add it (new terminals only):"
    Write-Host "  [Environment]::SetEnvironmentVariable('Path', `"$installDir;`$([Environment]::GetEnvironmentVariable('Path','User'))`", 'User')"
  }
} finally {
  Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
}

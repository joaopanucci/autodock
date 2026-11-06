<#
.SYNOPSIS
  AutoDock installer for Windows.

.DESCRIPTION
  irm https://raw.githubusercontent.com/joaopanucci/autodock/main/scripts/install.ps1 | iex

  Downloads the installer of the latest release, VERIFIES its SHA-256 against the
  release's SHA256SUMS.txt and installs it silently for the current user.

  Why this avoids the SmartScreen "Windows protected your PC" warning: SmartScreen
  only checks files that carry the Mark of the Web, which browsers add on download.
  PowerShell's Invoke-WebRequest does not add it. Nothing here weakens Windows: the
  installer is still unsigned, and you can read this whole script before running it.

.PARAMETER Version
  Release tag to install, for example v0.1.2 (default: latest). Also read from the
  AUTODOCK_VERSION environment variable.

.PARAMETER NoLaunch
  Do not open AutoDock after installing.
#>
param(
  [string]$Version = $env:AUTODOCK_VERSION,
  [switch]$NoLaunch
)

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'   # much faster downloads in Windows PowerShell 5.1
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12

$repo = 'joaopanucci/autodock'
$portuguese = (Get-Culture).Name -like 'pt*'
function Say([string]$pt, [string]$en) { if ($portuguese) { Write-Host $pt } else { Write-Host $en } }
function Fail([string]$pt, [string]$en) {
  if ($portuguese) { throw "Erro: $pt" } else { throw "Error: $en" }
}

if (-not $Version -or $Version -eq 'latest') {
  $release = Invoke-RestMethod -Uri "https://api.github.com/repos/$repo/releases/latest" `
    -Headers @{ 'User-Agent' = 'autodock-installer' }
  $tag = $release.tag_name
} else {
  $tag = $Version
}
# Only plain version tags: nothing else may end up inside the download URL.
if ($tag -notmatch '^v\d+\.\d+\.\d+$') { Fail "versão inválida: $tag" "invalid version: $tag" }

$asset = "AutoDock_$($tag.Substring(1))_x64-setup.exe"
$base = "https://github.com/$repo/releases/download/$tag"
$work = Join-Path ([IO.Path]::GetTempPath()) ("autodock-" + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $work | Out-Null

try {
  Say "Baixando AutoDock $tag..." "Downloading AutoDock $tag..."
  $installer = Join-Path $work $asset
  Invoke-WebRequest -UseBasicParsing -Uri "$base/$asset" -OutFile $installer
  $sums = Join-Path $work 'SHA256SUMS.txt'
  Invoke-WebRequest -UseBasicParsing -Uri "$base/SHA256SUMS.txt" -OutFile $sums

  Say 'Conferindo o checksum...' 'Verifying the checksum...'
  $expected = (Get-Content $sums | Where-Object { $_ -match "\s$([regex]::Escape($asset))$" } |
    ForEach-Object { ($_ -split '\s+')[0] } | Select-Object -First 1)
  if (-not $expected) { Fail "$asset não consta no SHA256SUMS.txt." "$asset is not listed in SHA256SUMS.txt." }
  $actual = (Get-FileHash -Algorithm SHA256 -Path $installer).Hash
  if ($expected.ToLowerInvariant() -ne $actual.ToLowerInvariant()) {
    Fail 'checksum diferente do esperado. Nada foi instalado.' 'checksum does not match. Nothing was installed.'
  }

  # Defensive: a PowerShell download has no Mark of the Web, but make sure.
  Unblock-File -Path $installer

  Say 'Instalando...' 'Installing...'
  $process = Start-Process -FilePath $installer -ArgumentList '/S' -Wait -PassThru
  if ($process.ExitCode -ne 0) { Fail "o instalador terminou com código $($process.ExitCode)." "the installer exited with code $($process.ExitCode)." }

  $exe = Get-ChildItem -Path (Join-Path $env:LOCALAPPDATA 'AutoDock'), (Join-Path $env:LOCALAPPDATA 'Programs\AutoDock') `
    -Filter 'autodock-desktop.exe' -Recurse -ErrorAction SilentlyContinue | Select-Object -First 1
  if (-not $exe) { Fail 'instalado, mas não encontrei o executável.' 'installed, but the executable was not found.' }

  Say "AutoDock $tag instalado em $($exe.DirectoryName)" "AutoDock $tag installed at $($exe.DirectoryName)"
  if (-not $NoLaunch) { Start-Process -FilePath $exe.FullName }
}
finally {
  Remove-Item -Recurse -Force -Path $work -ErrorAction SilentlyContinue
}

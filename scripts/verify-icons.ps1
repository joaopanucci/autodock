#requires -Version 7.0
param([switch]$CheckExecutable)
$ErrorActionPreference = 'Stop'
$root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
Add-Type -AssemblyName System.Drawing
$iconDir = Join-Path $root 'apps/desktop/src-tauri/icons'
$count = 0
foreach ($file in Get-ChildItem -LiteralPath $iconDir -Recurse -Filter '*.png') {
    $bitmap = [Drawing.Bitmap]::new($file.FullName)
    try {
        if ($bitmap.Width -lt 1 -or $bitmap.Height -lt 1) { throw "PNG inválido: $($file.Name)" }
        if ($file.BaseName -match '^(\d+)x(\d+)(@2x)?$') {
            $factor = if ($Matches[3]) { 2 } else { 1 }
            if ($bitmap.Width -ne ([int]$Matches[1] * $factor) -or $bitmap.Height -ne ([int]$Matches[2] * $factor)) { throw "Dimensão incorreta: $($file.Name)" }
        }
        $count++
    } finally { $bitmap.Dispose() }
}
$ico = [IO.File]::ReadAllBytes((Join-Path $iconDir 'icon.ico'))
if ($ico.Length -lt 6 -or $ico[0] -ne 0 -or $ico[1] -ne 0 -or $ico[2] -ne 1 -or $ico[3] -ne 0 -or [BitConverter]::ToUInt16($ico,4) -lt 1) { throw 'ICO inválido' }
$icns = [IO.File]::ReadAllBytes((Join-Path $iconDir 'icon.icns'))
if ([Text.Encoding]::ASCII.GetString($icns,0,4) -ne 'icns') { throw 'ICNS inválido' }
$nativeMatches = $null
if ($CheckExecutable) {
    $nativeIcon = [Drawing.Icon]::ExtractAssociatedIcon((Join-Path $root 'target/debug/autodock-desktop.exe'))
    $native = $nativeIcon.ToBitmap()
    $resource = [Drawing.Icon]::new((Join-Path $iconDir 'icon.ico'),$native.Width,$native.Height)
    $reference = $resource.ToBitmap()
    try {
        $nativeMatches = $native.Width -eq $reference.Width -and $native.Height -eq $reference.Height
        for ($y=0;$y -lt $native.Height -and $nativeMatches;$y++) {
            for ($x=0;$x -lt $native.Width;$x++) {
                if ($native.GetPixel($x,$y).ToArgb() -ne $reference.GetPixel($x,$y).ToArgb()) { $nativeMatches=$false; break }
            }
        }
        if (-not $nativeMatches) { throw 'O ícone extraído do executável difere do ICO gerado.' }
    } finally { $native.Dispose(); $reference.Dispose(); $resource.Dispose(); $nativeIcon.Dispose() }
}
[pscustomobject]@{ pngDecoded=$count; icoValid=$true; icnsHeaderValid=$true; executableMatchesGeneratedIco=$nativeMatches; sourceSha256=(Get-FileHash (Join-Path $iconDir 'app-icon.png')).Hash } | ConvertTo-Json

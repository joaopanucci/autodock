#requires -Version 7.0
[CmdletBinding()]
param([string]$Destination = $PSScriptRoot, [ValidateRange(100,60000)][int]$TimeoutMs = 8000)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
Set-StrictMode -Version Latest
if (-not $IsWindows) { throw 'Este bootstrap é para Windows; veja README para macOS.' }
. (Join-Path $PSScriptRoot 'scripts/Diagnostics.ps1')
$root = [IO.Path]::GetFullPath($Destination)
if ([IO.Path]::GetPathRoot($root) -eq $root) { throw 'A raiz da unidade não é um destino válido.' }
if (Test-Path -LiteralPath $root) {
    if ((Get-Item -LiteralPath $root).Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'Destino não pode ser link ou junction.' }
}
[IO.Directory]::CreateDirectory($root) | Out-Null
$reportBase = Join-Path $root '.autodock-bootstrap'
if ((Test-Path -LiteralPath $reportBase) -and ((Get-Item -LiteralPath $reportBase).Attributes -band [IO.FileAttributes]::ReparsePoint)) { throw 'Diretório de relatórios não pode ser link ou junction.' }
$runId = (Get-Date -Format 'yyyyMMdd-HHmmss-fff') + '-' + [guid]::NewGuid().ToString('N').Substring(0,8)
$reportDir = Join-Path $root ".autodock-bootstrap/$runId"
[IO.Directory]::CreateDirectory($reportDir) | Out-Null
$report = [ordered]@{timestamp=(Get-Date).ToString('o'); destination=$root; files=@(); checks=@(); notes=@('Não instala dependências; não executa Git remoto; Docker somente leitura.'); error=$null}
try {
    Write-Host "[ESTRUTURA] Preparando $root antes dos checks"
    $manifest = Get-Content -Raw -LiteralPath (Join-Path $PSScriptRoot 'bootstrap-files.json') | ConvertFrom-Json
    foreach ($relative in $manifest) {
        $source = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot $relative))
        $target = [IO.Path]::GetFullPath((Join-Path $root $relative))
        if (-not $source.StartsWith($PSScriptRoot.TrimEnd('\') + '\', [StringComparison]::OrdinalIgnoreCase) -or -not $target.StartsWith($root.TrimEnd('\') + '\', [StringComparison]::OrdinalIgnoreCase)) { throw "Caminho inválido no manifesto: $relative" }
        $ancestor = Split-Path $target
        while ($ancestor -and $ancestor.Length -ge $root.Length) {
            if ((Test-Path -LiteralPath $ancestor) -and ((Get-Item -LiteralPath $ancestor).Attributes -band [IO.FileAttributes]::ReparsePoint)) { throw "Junction/link bloqueado: $ancestor" }
            $ancestor = Split-Path $ancestor
        }
        if (Test-Path -LiteralPath $target) {
            $state = if ((Get-FileHash -LiteralPath $source).Hash -eq (Get-FileHash -LiteralPath $target).Hash) { 'preserved_identical' } else { 'preserved_different' }
            if ($state -eq 'preserved_different') { Write-Warning "Preservado sem sobrescrever: $relative" }
        } else {
            [IO.Directory]::CreateDirectory((Split-Path $target)) | Out-Null
            [IO.File]::Copy($source, $target, $false)
            $state = 'created'
        }
        $report.files += @{path=$relative; status=$state}
    }
    $specs = @(
        @{Name='pwsh'; Program=(Join-Path $PSHOME 'pwsh.exe')},
        @{Name='rustc'; Program='rustc'}, @{Name='cargo'; Program='cargo'},
        @{Name='node'; Program='node.exe'}, @{Name='npm'; Program='npm.cmd'},
        @{Name='git'; Program='git.exe'}, @{Name='docker'; Program='docker.exe'},
        @{Name='compose'; Program='docker.exe'; Arguments=@('compose','version')},
        @{Name='docker-engine'; Program='docker.exe'; Arguments=@('info','--format','{{.ServerVersion}}')},
        @{Name='msbuild'; Program='MSBuild.exe'; Arguments=@('-version','-nologo')}
    )
    foreach ($spec in $specs) {
        Write-Host "[CHECK] $($spec.Name) — limite $TimeoutMs ms"
        $check = Invoke-ToolProbe @spec -TimeoutMs $TimeoutMs -WorkingDirectory $root
        $report.checks += $check
        Write-Host "[$($check.status)] $($check.name) $($check.stdout) $($check.detail)"
    }
    $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio/Installer/vswhere.exe'
    if (Test-Path -LiteralPath $vswhere) {
        Write-Host '[CHECK] vswhere — instalação com ferramentas C++'
        $cpp = Invoke-ToolProbe -Name 'visual-studio-cpp' -Program $vswhere -Arguments @('-latest','-products','*','-requires','Microsoft.VisualStudio.Component.VC.Tools.x86.x64','-property','installationPath') -TimeoutMs $TimeoutMs
        if ($cpp.status -eq 'available' -and -not $cpp.stdout) { $cpp.status = 'missing'; $cpp.detail = 'vswhere não encontrou instalação com componente C++.' }
        $report.checks += $cpp
        Write-Host '[CHECK] vswhere — localizar MSBuild fora do PATH'
        $found = Invoke-ToolProbe -Name 'msbuild-discovery' -Program $vswhere -Arguments @('-latest','-products','*','-find','MSBuild\**\Bin\MSBuild.exe') -TimeoutMs $TimeoutMs
        $report.checks += $found
        if ($found.status -eq 'available' -and $found.stdout) {
            $msbuildPath = ($found.stdout -split '\r?\n')[0]
            if (Test-Path -LiteralPath $msbuildPath -PathType Leaf) {
                $report.checks = @($report.checks | Where-Object name -ne 'msbuild')
                $report.checks += Invoke-ToolProbe -Name 'msbuild' -Program $msbuildPath -Arguments @('-version','-nologo') -TimeoutMs $TimeoutMs
            }
        }
        $report.notes += 'vswhere exit 0 sem caminho não confirma ferramentas C++. MSBuild fora do PATH não prova ausência. WebView2/Windows SDK exigem validação separada do build.'
    }
} catch { $report.error = $_.Exception.Message; Write-Warning $report.error }
finally {
    $report | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath (Join-Path $reportDir 'diagnostic.json') -Encoding utf8
    @('# Diagnóstico AutoDock', '', "Destino: $root", '', ($report.checks | Format-Table name,status,exitCode,durationMs -AutoSize | Out-String), ($report.notes -join "`n"), "Erro: $($report.error)") | Set-Content -LiteralPath (Join-Path $reportDir 'diagnostic.md') -Encoding utf8
    Write-Host "[RELATÓRIO] $reportDir"
}
if ($report.error) { exit 1 }
if (@($report.checks | Where-Object status -ne 'available').Count) { Write-Warning 'Estrutura preservada. Ambiente com pendências; consulte o relatório.'; exit 2 }
Write-Host '[CONCLUÍDO] Estrutura e verificações concluídas.'

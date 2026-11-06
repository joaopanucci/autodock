#requires -Version 7.0
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
. (Join-Path $PSScriptRoot '../scripts/Diagnostics.ps1')
$sourceRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$destination = Join-Path $sourceRoot ('.autodock-bootstrap/bootstrap-tests-' + [guid]::NewGuid().ToString('N'))
$pwsh = Join-Path $PSHOME 'pwsh.exe'
$script = Join-Path $sourceRoot 'bootstrap-autodock.ps1'
function Run-Bootstrap {
    $result = Invoke-ToolProbe -Name 'bootstrap-test' -Program $pwsh -Arguments @('-NoProfile','-File',$script,'-Destination',$destination,'-TimeoutMs','2000') -TimeoutMs 60000
    if ($result.exitCode -notin @(0,2)) { throw "Bootstrap falhou: $($result | ConvertTo-Json -Depth 5)" }
}
Run-Bootstrap
if (-not (Test-Path -LiteralPath (Join-Path $destination 'Cargo.toml'))) { throw 'Workspace não criado com ferramentas faltantes' }
Write-Host 'PASS: estrutura criada antes das verificações, apesar das dependências ausentes'
$readme = Join-Path $destination 'README.md'
Add-Content -LiteralPath $readme 'USER CONTENT MUST SURVIVE'
$before = (Get-FileHash -LiteralPath $readme).Hash
Run-Bootstrap
if ((Get-FileHash -LiteralPath $readme).Hash -ne $before) { throw 'Arquivo alterado foi sobrescrito' }
Write-Host 'PASS: repetição preserva arquivo modificado'
$reports = @(Get-ChildItem -LiteralPath (Join-Path $destination '.autodock-bootstrap') -Recurse -Filter diagnostic.json)
if ($reports.Count -ne 2) { throw 'Esperados dois relatórios independentes' }
$latest = Get-Content -Raw ($reports | Sort-Object LastWriteTime | Select-Object -Last 1).FullName | ConvertFrom-Json
if (-not ($latest.files | Where-Object { $_.path -eq 'README.md' -and $_.status -eq 'preserved_different' })) { throw 'Conflito não reportado' }
Write-Host 'PASS: relatórios separados registram conflito sem sobrescrever'
Write-Host '3 testes aprovados. Artefatos mantidos no AutoDock.'

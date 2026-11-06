#requires -Version 7.0
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
. (Join-Path $PSScriptRoot '../scripts/Diagnostics.ps1')
$count = 0
function Assert($Condition, [string]$Message) { if (-not $Condition) { throw "FAIL: $Message" }; $script:count++; Write-Host "PASS: $Message" }
$pwsh = Join-Path $PSHOME 'pwsh.exe'
$result = Invoke-ToolProbe -Name 'missing' -Program 'autodock-no-such-command-829c4c'
Assert ($result.status -eq 'missing') 'Não resolvido é missing'
$result = Invoke-ToolProbe -Name 'success' -Program $pwsh -Arguments @('-NoProfile','-Command','[Console]::Out.Write("out"); [Console]::Error.Write("err")')
Assert ($result.status -eq 'available' -and $result.stdout -eq 'out' -and $result.stderr -eq 'err') 'Captura stdout/stderr e código zero'
$result = Invoke-ToolProbe -Name 'nonzero' -Program $pwsh -Arguments @('-NoProfile','-Command','exit 7')
Assert ($result.status -eq 'execution_error' -and $result.exitCode -eq 7) 'Código não zero é execution_error'
$result = Invoke-ToolProbe -Name 'timeout' -Program $pwsh -Arguments @('-NoProfile','-Command','Start-Sleep -Seconds 30') -TimeoutMs 300
Assert ($result.status -eq 'timeout' -and $result.durationMs -lt 4000) 'Timeout limitado'
$result = Invoke-ToolProbe -Name 'large-output' -Program $pwsh -Arguments @('-NoProfile','-Command','[Console]::Out.Write(("x" * 100000)); [Console]::Error.Write(("y" * 100000))')
Assert ($result.status -eq 'available' -and $result.stdout.Length -eq 100000 -and $result.stderr.Length -eq 100000) 'Saída acima do buffer não bloqueia'
$result = Invoke-ToolProbe -Name 'invalid-directory' -Program $pwsh -WorkingDirectory (Join-Path $PSScriptRoot 'nonexistent')
Assert ($result.status -eq 'execution_error') 'Falha ao iniciar não é missing'
$result = Invoke-ToolProbe -Name 'npm' -Program 'npm.cmd'
Assert ($result.status -eq 'available' -or $result.status -eq 'missing') 'npm é resolvido via node, sem executar cmd como binário'
$testDir = Join-Path $PSScriptRoot ('../.autodock-bootstrap/probe-tests-' + [guid]::NewGuid().ToString('N'))
[IO.Directory]::CreateDirectory($testDir) | Out-Null
$childPidFile = Join-Path $testDir 'child.pid'
$script = '$child = Start-Process -FilePath ' + "'$pwsh'" + ' -ArgumentList @(''-NoProfile'',''-Command'',''Start-Sleep -Seconds 30'') -WindowStyle Hidden -PassThru; [IO.File]::WriteAllText(' + "'$childPidFile'" + ', [string]$child.Id); Start-Sleep -Seconds 30'
$encoded = [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($script))
$result = Invoke-ToolProbe -Name 'process-tree' -Program $pwsh -Arguments @('-NoProfile','-EncodedCommand',$encoded) -TimeoutMs 2000
Assert ($result.status -eq 'timeout' -and (Test-Path -LiteralPath $childPidFile)) 'Teste criou filho e atingiu timeout'
$childId = [int](Get-Content -LiteralPath $childPidFile)
Start-Sleep -Milliseconds 200
Assert (-not (Get-Process -Id $childId -ErrorAction SilentlyContinue)) 'Timeout encerra também o processo filho'
$argumentScript = Join-Path $testDir 'probe with spaces.ps1'
'param([string]$Value) [Console]::OutputEncoding = [Text.UTF8Encoding]::new($false); [Console]::Write($Value)' | Set-Content -LiteralPath $argumentScript -Encoding utf8
$result = Invoke-ToolProbe -Name 'arguments' -Program $pwsh -Arguments @('-NoProfile','-File',$argumentScript,'argumento com espaços')
Assert ($result.status -eq 'available' -and $result.stdout -eq 'argumento com espaços') 'Argumentos com espaços preservados'
Write-Host "$count testes aprovados."

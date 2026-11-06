#requires -Version 7.0
Set-StrictMode -Version Latest
if ($IsWindows -and -not ('AutoDock.NativeJob' -as [type])) {
    Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
namespace AutoDock {
    public static class NativeJob {
        [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
        public static extern IntPtr CreateJobObject(IntPtr attributes, string name);
        [DllImport("kernel32.dll", SetLastError = true)]
        [return: MarshalAs(UnmanagedType.Bool)]
        public static extern bool AssignProcessToJobObject(IntPtr job, IntPtr process);
        [DllImport("kernel32.dll", SetLastError = true)]
        [return: MarshalAs(UnmanagedType.Bool)]
        public static extern bool TerminateJobObject(IntPtr job, uint code);
        [DllImport("kernel32.dll")]
        [return: MarshalAs(UnmanagedType.Bool)]
        public static extern bool CloseHandle(IntPtr handle);
    }
}
'@
}
function Invoke-ToolProbe {
    param(
        [Parameter(Mandatory)][string]$Name,
        [Parameter(Mandatory)][string]$Program,
        [string[]]$Arguments = @('--version'),
        [ValidateRange(100,3600000)][int]$TimeoutMs = 8000,
        [string]$WorkingDirectory = $PWD.Path
    )
    $watch = [Diagnostics.Stopwatch]::StartNew()
    $result = [ordered]@{name=$Name; executable=$null; arguments=$Arguments; status='missing'; exitCode=$null; stdout=''; stderr=''; detail='Não resolvido no PATH ou locais conhecidos; instalação pode existir.'; durationMs=0}
    $resolved = Get-Command $Program -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1
    if (-not $resolved -and $Program -in @('cargo','rustc')) {
        $cargoRoot = if ($env:CARGO_HOME) { $env:CARGO_HOME } else { Join-Path $env:USERPROFILE '.cargo' }
        $candidate = Join-Path $cargoRoot "bin/$Program.exe"
        if (Test-Path -LiteralPath $candidate -PathType Leaf) { $resolved = Get-Command $candidate -CommandType Application }
    }
    if (-not $resolved) { return [pscustomobject]$result }
    $result.executable = $resolved.Source
    $result.status = 'execution_error'
    $result.detail = ''
    $proc = [Diagnostics.Process]::new()
    $job = [IntPtr]::Zero
    try {
        $info = [Diagnostics.ProcessStartInfo]::new()
        $info.FileName = $resolved.Source
        $info.WorkingDirectory = $WorkingDirectory
        $info.UseShellExecute = $false
        $info.CreateNoWindow = $true
        $info.RedirectStandardOutput = $true
        $info.RedirectStandardError = $true
        $info.StandardOutputEncoding = [Text.UTF8Encoding]::new($false)
        $info.StandardErrorEncoding = [Text.UTF8Encoding]::new($false)
        $info.RedirectStandardInput = $true
        if ([IO.Path]::GetExtension($resolved.Source) -in @('.cmd','.bat')) {
            # No shell string interpolation: npm is launched as node + a literal JS path.
            $npmScript = Join-Path (Split-Path $resolved.Source) 'node_modules/npm/bin/npm-cli.js'
            $node = Get-Command node.exe -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1
            if ($Name -ne 'npm' -or -not $node -or -not (Test-Path -LiteralPath $npmScript)) { throw 'Wrapper batch encontrado, mas entrada nativa npm/node não resolvida.' }
            $info.FileName = $node.Source
            $info.ArgumentList.Add($npmScript)
        }
        foreach ($argument in $Arguments) { $info.ArgumentList.Add($argument) }
        $proc.StartInfo = $info
        $job = [AutoDock.NativeJob]::CreateJobObject([IntPtr]::Zero, $null)
        if ($job -eq [IntPtr]::Zero) { throw 'Não foi possível criar Job Object para a verificação.' }
        [void]$proc.Start()
        if (-not [AutoDock.NativeJob]::AssignProcessToJobObject($job, $proc.Handle) -and -not $proc.HasExited) {
            throw 'Não foi possível associar processo ao Job Object.'
        }
        $proc.StandardInput.Close()
        # Drain both streams BEFORE waiting. Includes EOF in the deadline.
        $outTask = $proc.StandardOutput.ReadToEndAsync()
        $errTask = $proc.StandardError.ReadToEndAsync()
        while ($watch.ElapsedMilliseconds -lt $TimeoutMs) {
            if ($proc.HasExited -and $outTask.IsCompleted -and $errTask.IsCompleted) { break }
            Start-Sleep -Milliseconds 20
        }
        if (-not $proc.HasExited -or -not $outTask.IsCompleted -or -not $errTask.IsCompleted) {
            $result.status = 'timeout'
            $result.detail = "Prazo de $TimeoutMs ms excedido (processo ou captura de saída)."
            if (-not [AutoDock.NativeJob]::TerminateJobObject($job, 124)) { $result.detail += ' Falha ao encerrar Job Object.' }
            if (-not $proc.HasExited) {
                try { $proc.Kill($true); [void]$proc.WaitForExit(1500) } catch { $result.detail += " Falha ao encerrar árvore: $($_.Exception.Message)" }
            } else { $result.detail += ' Processo principal saiu; grupo encerrado para liberar filhos e pipes.' }
        } else {
            $result.exitCode = $proc.ExitCode
            $result.status = if ($proc.ExitCode -eq 0) { 'available' } else { 'execution_error' }
        }
        if ($outTask.IsCompletedSuccessfully) { $result.stdout = $outTask.Result.Trim() }
        if ($errTask.IsCompletedSuccessfully) { $result.stderr = $errTask.Result.Trim() }
    } catch {
        $result.status = 'execution_error'
        $result.detail = $_.Exception.Message
        try { if ($proc.Id -and -not $proc.HasExited) { $proc.Kill($true); [void]$proc.WaitForExit(1500) } } catch { }
    } finally {
        if ($job -ne [IntPtr]::Zero) {
            [void][AutoDock.NativeJob]::TerminateJobObject($job, 0)
            [void][AutoDock.NativeJob]::CloseHandle($job)
        }
        $proc.Dispose()
        $result.durationMs = $watch.ElapsedMilliseconds
    }
    [pscustomobject]$result
}

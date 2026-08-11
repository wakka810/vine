[CmdletBinding()]
param(
    [ValidateRange(1, 20)]
    [int]$Runs = 3
)

$ErrorActionPreference = 'Stop'

$repositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$probeRoot = Join-Path $repositoryRoot '.work\show-cursor-thread-local-oracle'
$sourcePath = Join-Path $probeRoot 'ShowCursorThreadLocal.cs'
$executablePath = Join-Path $probeRoot 'ShowCursorThreadLocal.exe'
$compilerPath = Join-Path $env:WINDIR 'Microsoft.NET\Framework\v4.0.30319\csc.exe'

if (-not (Test-Path -LiteralPath $compilerPath -PathType Leaf)) {
    throw "32-bit .NET Framework C# compiler was not found at $compilerPath"
}

New-Item -ItemType Directory -Path $probeRoot -Force | Out-Null

$source = @'
using System;
using System.Runtime.InteropServices;
using System.Threading;

internal static class ShowCursorThreadLocal
{
    [DllImport("user32.dll")]
    private static extern int ShowCursor(bool show);

    private static int workerShow;
    private static int workerRestore;
    private static Exception workerFailure;

    private static void ProbeWorker()
    {
        try
        {
            workerShow = ShowCursor(true);
            workerRestore = ShowCursor(false);
        }
        catch (Exception exception)
        {
            workerFailure = exception;
        }
    }

    private static int Main()
    {
        int mainHide = ShowCursor(false);
        Thread worker = new Thread(ProbeWorker);
        worker.Start();
        worker.Join();
        int mainRestore = ShowCursor(true);

        if (workerFailure != null)
        {
            Console.Error.WriteLine(workerFailure);
            return 2;
        }

        Console.WriteLine(
            String.Format("{0},{1},{2},{3}", mainHide, workerShow, workerRestore, mainRestore));
        return 0;
    }
}
'@

Set-Content -LiteralPath $sourcePath -Value $source -Encoding utf8
& $compilerPath /nologo /target:exe /platform:x86 "/out:$executablePath" $sourcePath
if ($LASTEXITCODE -ne 0) {
    throw "ShowCursor thread-local oracle compilation failed with exit code $LASTEXITCODE"
}

$observations = @()
for ($run = 1; $run -le $Runs; $run++) {
    $observation = (& $executablePath).Trim()
    if ($LASTEXITCODE -ne 0) {
        throw "ShowCursor thread-local oracle run $run failed with exit code $LASTEXITCODE"
    }
    if ($observation -ne '-1,1,0,0') {
        throw "ShowCursor thread-local oracle run $run returned '$observation'; expected '-1,1,0,0'"
    }
    $observations += $observation
}

[ordered]@{
    schemaVersion = 1
    architecture = 'x86'
    runs = $Runs
    expected = '-1,1,0,0'
    observations = $observations
    countersRestored = $true
} | ConvertTo-Json -Depth 3

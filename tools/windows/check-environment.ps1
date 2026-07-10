param(
    [string]$BgiExe = "",
    [switch]$SourceAdapter
)

$ErrorActionPreference = "Stop"

function Invoke-CheckedNative {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Command,
        [string[]]$Arguments = @()
    )

    $output = & $Command @Arguments 2>&1
    $exitCode = $LASTEXITCODE
    if ($null -eq $exitCode -or $exitCode -ne 0) {
        $argumentText = $Arguments -join " "
        throw "native command failed with exit code ${exitCode}: $Command $argumentText"
    }
    return $output
}

$required = @("git", "node", "npm", "clang", "lld-link", "wasm-ld", "llvm-readobj")
if ($SourceAdapter) {
    $required += "make", "sh"
}

foreach ($command in $required) {
    $resolved = Get-Command $command -ErrorAction SilentlyContinue
    if ($null -eq $resolved) {
        throw "missing command: $command"
    }
    Write-Host ("{0}: {1}" -f $command, $resolved.Source)
}

$python = Get-Command py -ErrorAction SilentlyContinue
if ($null -eq $python) {
    $python = Get-Command python -ErrorAction SilentlyContinue
}
if ($null -eq $python) {
    throw "missing command: py or python"
}
Write-Host ("python: {0}" -f $python.Source)

Invoke-CheckedNative -Command "git" -Arguments @("--version")
Invoke-CheckedNative -Command "node" -Arguments @("--version")
Invoke-CheckedNative -Command "npm" -Arguments @("--version")
Invoke-CheckedNative -Command $python.Source -Arguments @("--version")
Invoke-CheckedNative -Command "clang" -Arguments @("--version") |
    Select-Object -First 1
Invoke-CheckedNative -Command "lld-link" -Arguments @("--version")
Invoke-CheckedNative -Command "wasm-ld" -Arguments @("--version")
Invoke-CheckedNative -Command "llvm-readobj" -Arguments @("--version") |
    Select-String "LLVM version" |
    Select-Object -First 1
Invoke-CheckedNative -Command $python.Source -Arguments @(
    "-c", "import frida; print('frida:', frida.__version__)"
)
$fridaTools = Invoke-CheckedNative -Command $python.Source -Arguments @(
    "-m", "pip", "show", "frida-tools"
) |
    Select-String "^Version:" |
    Select-Object -First 1
if ($null -eq $fridaTools) {
    throw "frida-tools package version not found"
}
Write-Host ("frida-tools: {0}" -f $fridaTools.Line.Split(":", 2)[1].Trim())

$windows = Get-CimInstance Win32_OperatingSystem
$ubr = (Get-ItemProperty "HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion").UBR
Write-Host ("Windows: {0}, {1}.{2}, {3}" -f `
        $windows.Caption, $windows.Version, $ubr, $windows.OSArchitecture)

$chromeCandidates = @(
    "C:\Program Files\Google\Chrome\Application\chrome.exe",
    "${env:LOCALAPPDATA}\Google\Chrome\Application\chrome.exe"
)
$chrome = $chromeCandidates |
    Where-Object { Test-Path -LiteralPath $_ -PathType Leaf } |
    Select-Object -First 1
if ($null -eq $chrome) {
    throw "Chrome executable not found"
}
$chromeVersion = (Get-Item -LiteralPath $chrome).VersionInfo.ProductVersion
Write-Host ("Chrome: {0} ({1})" -f $chromeVersion, $chrome)

if ($BgiExe -ne "") {
    if (-not (Test-Path -LiteralPath $BgiExe -PathType Leaf)) {
        throw "BGI.exe not found: $BgiExe"
    }
    $expected = "f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1"
    $actual = (Get-FileHash -LiteralPath $BgiExe -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($actual -ne $expected) {
        throw "BGI.exe hash mismatch: $actual"
    }
    $size = (Get-Item -LiteralPath $BgiExe).Length
    Write-Host ("BGI.exe: {0} bytes, SHA-256 {1}, fingerprint matched" -f `
            $size, $actual)
}

Write-Host "Windows development environment check passed"

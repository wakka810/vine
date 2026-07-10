param(
    [string]$BgiExe = ""
)

$ErrorActionPreference = "Stop"
$required = @("git", "node", "npm", "clang", "lld-link", "wasm-ld", "llvm-readobj")

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

node --version
npm --version
& $python.Source --version
clang --version | Select-Object -First 1
& $python.Source -c "import frida; print('frida:', frida.__version__)"

if ($BgiExe -ne "") {
    if (-not (Test-Path -LiteralPath $BgiExe -PathType Leaf)) {
        throw "BGI.exe not found: $BgiExe"
    }
    $expected = "f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1"
    $actual = (Get-FileHash -LiteralPath $BgiExe -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($actual -ne $expected) {
        throw "BGI.exe hash mismatch: $actual"
    }
    Write-Host "BGI.exe: fingerprint matched"
}

Write-Host "Windows development environment check passed"

[CmdletBinding()]
param(
    [Parameter(Position = 0, ValueFromRemainingArguments = $true)]
    [string[]]$CargoArgs
)

Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'

$sccache = Get-Command 'sccache' -ErrorAction SilentlyContinue
if ($null -eq $sccache) {
    [Console]::Error.WriteLine('sccache is required by cargo-agent.ps1 but was not found on PATH. Run cargo directly to build without the optional compiler cache.')
    exit 127
}

$cargo = Get-Command 'cargo' -ErrorAction SilentlyContinue
if ($null -eq $cargo) {
    [Console]::Error.WriteLine('cargo was not found on PATH.')
    exit 127
}

if ([string]::IsNullOrWhiteSpace($env:LOCALAPPDATA)) {
    [Console]::Error.WriteLine('LOCALAPPDATA is required to locate the user-local sccache directory.')
    exit 2
}

# Do not assign CARGO_TARGET_DIR here. Cargo's default remains this worktree's
# target directory, and an explicitly configured private target remains intact.
$previousSccacheDir = [Environment]::GetEnvironmentVariable('SCCACHE_DIR', 'Process')
$previousIncremental = [Environment]::GetEnvironmentVariable('CARGO_INCREMENTAL', 'Process')
$previousWrapper = [Environment]::GetEnvironmentVariable('RUSTC_WRAPPER', 'Process')
$cargoExitCode = 1

try {
    $env:SCCACHE_DIR = Join-Path $env:LOCALAPPDATA 'govfolio\sccache'
    $env:CARGO_INCREMENTAL = '0'
    $env:RUSTC_WRAPPER = $sccache.Source

    & $cargo.Source @CargoArgs
    $cargoExitCode = $LASTEXITCODE

    & $sccache.Source --show-stats
} finally {
    [Environment]::SetEnvironmentVariable('SCCACHE_DIR', $previousSccacheDir, 'Process')
    [Environment]::SetEnvironmentVariable('CARGO_INCREMENTAL', $previousIncremental, 'Process')
    [Environment]::SetEnvironmentVariable('RUSTC_WRAPPER', $previousWrapper, 'Process')
}
exit $cargoExitCode

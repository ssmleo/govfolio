Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'
$CargoArgs = @($args)

$sccache = Get-Command 'sccache' -ErrorAction SilentlyContinue
if ($null -eq $sccache) {
    [Console]::Error.WriteLine('sccache is required by cargo-agent.ps1 but was not found on PATH. Install sccache before using this managed wrapper.')
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

$repo = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
if ([string]::IsNullOrWhiteSpace($env:GOVFOLIO_LOOP_BIN)) {
    $targetDir = if ([string]::IsNullOrWhiteSpace($env:CARGO_TARGET_DIR)) {
        Join-Path $repo 'target'
    } elseif ([IO.Path]::IsPathRooted($env:CARGO_TARGET_DIR)) {
        $env:CARGO_TARGET_DIR
    } else {
        Join-Path $repo $env:CARGO_TARGET_DIR
    }
    $loopBin = Join-Path $targetDir 'debug\govfolio-loop.exe'
} else {
    $loopBin = $env:GOVFOLIO_LOOP_BIN
}
if (-not (Test-Path -LiteralPath $loopBin -PathType Leaf)) {
    [Console]::Error.WriteLine("pre-built govfolio-loop is required at $loopBin. Start 'govfolio-loop run' or 'govfolio-loop serve-builds' before managed Cargo work.")
    exit 75
}

try {
    $env:SCCACHE_DIR = Join-Path $env:LOCALAPPDATA 'govfolio\sccache'
    $env:CARGO_INCREMENTAL = '0'
    $env:RUSTC_WRAPPER = $sccache.Source

    & $loopBin 'cargo' '--' @CargoArgs
    $cargoExitCode = $LASTEXITCODE

    & $sccache.Source --show-stats
} finally {
    [Environment]::SetEnvironmentVariable('SCCACHE_DIR', $previousSccacheDir, 'Process')
    [Environment]::SetEnvironmentVariable('CARGO_INCREMENTAL', $previousIncremental, 'Process')
    [Environment]::SetEnvironmentVariable('RUSTC_WRAPPER', $previousWrapper, 'Process')
}
exit $cargoExitCode

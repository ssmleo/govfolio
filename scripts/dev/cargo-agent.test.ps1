[CmdletBinding()]
param()

Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'

$root = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$wrapper = Join-Path $PSScriptRoot 'cargo-agent.ps1'
$wrapperText = [IO.File]::ReadAllText($wrapper)
if ($wrapperText -match 'Run cargo directly') {
    throw 'cargo-agent.ps1 must not recommend bypassing supervisor admission'
}
$temp = Join-Path ([IO.Path]::GetTempPath()) ('govfolio-cargo-agent-test-' + [guid]::NewGuid().ToString('N'))
$oldPath = $env:PATH
$oldLoop = [Environment]::GetEnvironmentVariable('GOVFOLIO_LOOP_BIN', 'Process')
$oldLocalAppData = [Environment]::GetEnvironmentVariable('LOCALAPPDATA', 'Process')

try {
    [IO.Directory]::CreateDirectory($temp) | Out-Null
    $directMarker = Join-Path $temp 'direct-cargo.txt'
    $loopMarker = Join-Path $temp 'loop-args.txt'
    [IO.File]::WriteAllText(
        (Join-Path $temp 'sccache.cmd'),
        "@echo off`r`nexit /b 0`r`n"
    )
    [IO.File]::WriteAllText(
        (Join-Path $temp 'cargo.cmd'),
        "@echo off`r`n>`"$directMarker`" echo direct`r`nexit /b 0`r`n"
    )
    [IO.File]::WriteAllText(
        (Join-Path $temp 'govfolio-loop.cmd'),
        "@echo off`r`n>`"$loopMarker`" echo %*`r`nexit /b 37`r`n"
    )

    $env:PATH = "$temp;$oldPath"
    $env:GOVFOLIO_LOOP_BIN = Join-Path $temp 'govfolio-loop.cmd'
    $env:LOCALAPPDATA = $temp

    $process = Start-Process -FilePath 'powershell.exe' -ArgumentList @(
        '-NoProfile',
        '-NonInteractive',
        '-File',
        $wrapper,
        'check',
        '-p',
        'core'
    ) -Wait -PassThru -WindowStyle Hidden

    if ($process.ExitCode -ne 37) {
        throw "cargo-agent.ps1 returned $($process.ExitCode), expected supervisor exit 37"
    }
    if (Test-Path -LiteralPath $directMarker) {
        throw 'cargo-agent.ps1 invoked Cargo directly instead of the supervisor client'
    }
    $actual = [IO.File]::ReadAllText($loopMarker).Trim()
    if ($actual -ne 'cargo -- check -p core') {
        throw "unexpected supervisor arguments: $actual"
    }
} finally {
    $env:PATH = $oldPath
    [Environment]::SetEnvironmentVariable('GOVFOLIO_LOOP_BIN', $oldLoop, 'Process')
    [Environment]::SetEnvironmentVariable('LOCALAPPDATA', $oldLocalAppData, 'Process')
    if (Test-Path -LiteralPath $temp) {
        Remove-Item -LiteralPath $temp -Recurse -Force
    }
}

Write-Output 'cargo-agent.ps1 supervisor delegation contract passed'

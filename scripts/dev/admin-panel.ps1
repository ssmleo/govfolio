# admin-panel.ps1 - run/stop the admin observability dashboard (api + web) on
# the Windows dev host, goal 091.
#
#   .\scripts\dev\admin-panel.ps1 run     # start postgres + api + web (idempotent)
#   .\scripts\dev\admin-panel.ps1 stop    # stop api + web, sweep both ports for orphans
#   .\scripts\dev\admin-panel.ps1 status  # report postgres / api / web port state
#
# PowerShell 5.1-compatible. User-scope only, no admin required.
#
# Does NOT stop Postgres on `stop` - it's shared dev infra other work may
# depend on; run `.\scripts\dev\pg-local.ps1 stop` explicitly for that.
#
# Orphan handling: `cargo run` / `pnpm dev` are supervisor processes whose
# real listener is a CHILD process (api.exe / node.exe) - killing only the
# tracked launcher PID can leave that child bound to the port. `stop`
# therefore treats the PID file as a hint, not the source of truth: it kills
# the tracked PID *and* unconditionally sweeps whatever process is actually
# LISTENing on each port (Get-NetTCPConnection -> OwningProcess), which is
# correct regardless of process-tree depth or how the process got there.

param(
    [Parameter(Position = 0)]
    [ValidateSet('run', 'stop', 'status')]
    [string]$Command = 'status'
)

$ErrorActionPreference = 'Stop'

$RepoRoot   = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$StateDir   = Join-Path $env:LOCALAPPDATA 'govfolio\admin-panel'
$ApiPort    = 8080
$WebPort    = 3000
$AdminToken = 'dev-admin-token'
$DatabaseUrl = 'postgres://postgres:postgres@localhost:5433/govfolio'

New-Item -ItemType Directory -Force $StateDir | Out-Null
$ApiPidFile = Join-Path $StateDir 'api.pid'
$WebPidFile = Join-Path $StateDir 'web.pid'
$ApiLogFile = Join-Path $StateDir 'api.log'
$WebLogFile = Join-Path $StateDir 'web.log'

function Test-PortOpen($Port) {
    $client = New-Object System.Net.Sockets.TcpClient
    try {
        $async = $client.BeginConnect('127.0.0.1', $Port, $null, $null)
        if ($async.AsyncWaitHandle.WaitOne(1000) -and $client.Connected) { return $true }
        return $false
    } catch {
        return $false
    } finally {
        $client.Close()
    }
}

function Wait-Port($Port, $Label, $TimeoutSeconds = 60) {
    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    while ((Get-Date) -lt $deadline) {
        if (Test-PortOpen $Port) { Write-Host "$Label`: listening on :$Port"; return }
        Start-Sleep -Seconds 1
    }
    Write-Error "$Label`: timed out waiting for :$Port (see the log file for details)"
    exit 1
}

function Stop-ByPidFile($PidFile, $Label) {
    if (-not (Test-Path $PidFile)) { return }
    $procId = Get-Content $PidFile -ErrorAction SilentlyContinue
    if ($procId) {
        Stop-Process -Id $procId -Force -ErrorAction SilentlyContinue
        if ($?) { Write-Host "$Label`: stopped tracked pid $procId" }
    }
    Remove-Item $PidFile -ErrorAction SilentlyContinue
}

function Stop-ByPort($Port, $Label) {
    $owners = Get-NetTCPConnection -LocalPort $Port -State Listen -ErrorAction SilentlyContinue |
        Select-Object -ExpandProperty OwningProcess -Unique
    foreach ($procId in $owners) {
        Write-Host "$Label`: killing orphan pid $procId listening on :$Port"
        Stop-Process -Id $procId -Force -ErrorAction SilentlyContinue
    }
}

switch ($Command) {
    'run' {
        & (Join-Path $PSScriptRoot 'pg-local.ps1') start

        # Launching two processes is not atomic by default: if starting api
        # succeeds but starting web then throws, api would be left running
        # with no PID file pointing at it (an untracked orphan) unless we
        # clean up explicitly on the way out.
        try {
            if (Test-PortOpen $ApiPort) {
                Write-Host "api: already listening on :$ApiPort"
            } else {
                $env:DATABASE_URL = $DatabaseUrl
                $env:ADMIN_TOKEN = $AdminToken
                $env:GOVFOLIO_REPO_ROOT = $RepoRoot
                $p = Start-Process -FilePath 'cargo' -ArgumentList @('run', '-p', 'api') `
                    -WorkingDirectory $RepoRoot -WindowStyle Hidden -PassThru `
                    -RedirectStandardOutput $ApiLogFile -RedirectStandardError "$ApiLogFile.err"
                $p.Id | Out-File -Encoding ascii $ApiPidFile
                Write-Host "api: starting (pid $($p.Id), log $ApiLogFile)"
            }

            if (Test-PortOpen $WebPort) {
                Write-Host "web: already listening on :$WebPort"
            } else {
                $env:GOVFOLIO_ADMIN_TOKEN = $AdminToken
                $env:GOVFOLIO_API_URL = "http://localhost:$ApiPort"
                # pnpm is a .cmd shim on Windows, not a native exe - Start-Process
                # -FilePath can't CreateProcess it directly ("not a valid Win32
                # application"); route through cmd.exe /c, which resolves PATHEXT.
                $p = Start-Process -FilePath 'cmd.exe' -ArgumentList @('/c', 'pnpm', '--filter', 'web', 'dev') `
                    -WorkingDirectory $RepoRoot -WindowStyle Hidden -PassThru `
                    -RedirectStandardOutput $WebLogFile -RedirectStandardError "$WebLogFile.err"
                $p.Id | Out-File -Encoding ascii $WebPidFile
                Write-Host "web: starting (pid $($p.Id), log $WebLogFile)"
            }
        } catch {
            Write-Error "run: failed to launch ($_) - cleaning up anything already started"
            Stop-ByPidFile $ApiPidFile 'api'
            Stop-ByPidFile $WebPidFile 'web'
            Stop-ByPort $ApiPort 'api'
            Stop-ByPort $WebPort 'web'
            exit 1
        }

        Wait-Port $ApiPort 'api'
        Wait-Port $WebPort 'web'
        Write-Host ''
        Write-Host "admin dashboard: http://localhost:$WebPort/admin"
    }
    'stop' {
        Stop-ByPidFile $WebPidFile 'web'
        Stop-ByPidFile $ApiPidFile 'api'
        Stop-ByPort $WebPort 'web'
        Stop-ByPort $ApiPort 'api'
        Write-Host 'admin dashboard stopped (postgres left running - see pg-local.ps1 stop)'
    }
    'status' {
        & (Join-Path $PSScriptRoot 'pg-local.ps1') status
        if (Test-PortOpen $ApiPort) { Write-Host "api :$ApiPort`: up" } else { Write-Host "api :$ApiPort`: down" }
        if (Test-PortOpen $WebPort) { Write-Host "web :$WebPort`: up" } else { Write-Host "web :$WebPort`: down" }
    }
}

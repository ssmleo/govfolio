# pg-local.ps1 - manage the portable PostgreSQL 16 instance on the Windows dev host.
#
#   .\scripts\dev\pg-local.ps1 start    # start server (no-op if already running)
#   .\scripts\dev\pg-local.ps1 stop     # fast shutdown
#   .\scripts\dev\pg-local.ps1 status   # pg_ctl status + port probe
#
# Layout (see docs/runbooks/dev-host-windows.md):
#   binaries  %LOCALAPPDATA%\govfolio\pg16\bin   (zonky bundle: initdb/pg_ctl/postgres only)
#   data      %LOCALAPPDATA%\govfolio\data
#   log       %LOCALAPPDATA%\govfolio\pg.log
#   port      5433 (passed via -o; postgresql.conf keeps defaults), trust auth, db govfolio
#
# PowerShell 5.1-compatible. User-scope only, no admin required.

param(
    [Parameter(Position = 0)]
    [ValidateSet('start', 'stop', 'status')]
    [string]$Command = 'status'
)

$ErrorActionPreference = 'Stop'

$PgRoot  = Join-Path $env:LOCALAPPDATA 'govfolio'
$PgCtl   = Join-Path $PgRoot 'pg16\bin\pg_ctl.exe'
$DataDir = Join-Path $PgRoot 'data'
$LogFile = Join-Path $PgRoot 'pg.log'
$Port    = 5433

function Test-PgPort {
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

function Test-PgRunning {
    # pg_ctl status exits 0 when a postmaster is running in $DataDir.
    & $PgCtl status -D $DataDir *> $null
    return ($LASTEXITCODE -eq 0)
}

if (-not (Test-Path $PgCtl))   { Write-Error "pg_ctl not found at $PgCtl"; exit 1 }
if (-not (Test-Path (Join-Path $DataDir 'PG_VERSION'))) {
    Write-Error "No PostgreSQL cluster at $DataDir (see runbook 'Recovery' to rebuild)"; exit 1
}

switch ($Command) {
    'start' {
        if (Test-PgRunning) {
            Write-Host "already running (data: $DataDir, port $Port)"
            exit 0
        }
        if (Test-PgPort) {
            Write-Error "port $Port is in use but pg_ctl reports no server in $DataDir - refusing to start"
            exit 1
        }
        & $PgCtl start -D $DataDir -l $LogFile -o "-p $Port" -w -t 30
        if ($LASTEXITCODE -ne 0) {
            Write-Error "pg_ctl start failed (exit $LASTEXITCODE) - check $LogFile"
            exit 1
        }
        if (Test-PgPort) {
            Write-Host "started: 127.0.0.1:$Port (log: $LogFile)"
            exit 0
        }
        Write-Error "server started but port $Port not answering - check $LogFile"
        exit 1
    }
    'stop' {
        if (-not (Test-PgRunning)) {
            Write-Host 'not running'
            exit 0
        }
        & $PgCtl stop -D $DataDir -m fast -w -t 30
        if ($LASTEXITCODE -ne 0) {
            Write-Error "pg_ctl stop failed (exit $LASTEXITCODE)"
            exit 1
        }
        Write-Host 'stopped'
        exit 0
    }
    'status' {
        & $PgCtl status -D $DataDir
        $ctlExit = $LASTEXITCODE
        if (Test-PgPort) {
            Write-Host "port ${Port}: accepting connections"
        } else {
            Write-Host "port ${Port}: not answering"
        }
        exit $ctlExit
    }
}

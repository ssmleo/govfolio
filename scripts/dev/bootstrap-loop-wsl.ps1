param(
    [Parameter(Position = 0)]
    [ValidateSet("status", "install", "verify")]
    [string]$Command = "status",
    [string]$DistroName = "Ubuntu-24.04",
    [string]$LoopUser = "govfolio-loop",
    [string]$LaneRoot = "/home/govfolio-loop/govfolio-lanes",
    [string]$NativeUnsupportedProof,
    [switch]$ConfirmInstall,
    [Parameter(DontShow = $true)]
    [string]$WslExecutable = "wsl.exe"
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

function Assert-SafeName([string]$Value, [string]$Label) {
    if ($Value -notmatch '^[A-Za-z0-9._-]+$') { throw "$Label contains unsupported characters" }
}

function Assert-SafeLinuxPath([string]$Value) {
    if ($Value -notmatch '^/[A-Za-z0-9._/-]+$' -or $Value.Contains("..")) {
        throw "LaneRoot must be an absolute, traversal-free Linux path"
    }
}

function Assert-WorkerDistro([string]$Name) {
    if ($Name -match '(?i)^docker-desktop(?:-data)?$') {
        throw "docker-desktop distributions are never valid loop workers"
    }
}

function Get-WslCommand {
    $resolved = Get-Command $WslExecutable -ErrorAction SilentlyContinue
    if ($null -eq $resolved) { return $null }
    return $resolved.Source
}

function Invoke-Wsl([string[]]$Arguments) {
    $output = & $script:ResolvedWsl @Arguments 2>&1
    $exitCode = $LASTEXITCODE
    $text = (($output | ForEach-Object { $_.ToString() }) -join "`n").Replace([string][char]0, '')
    return [PSCustomObject]@{ ExitCode = $exitCode; Output = $text.Trim() }
}

function Get-Distros {
    $result = Invoke-Wsl @("--list", "--verbose")
    if ($result.ExitCode -ne 0) {
        if ($result.Output -match '(?i)no installed distributions') { return @() }
        throw "wsl --list --verbose failed: $($result.Output)"
    }
    $distros = @()
    foreach ($line in ($result.Output -split "`r?`n")) {
        if ($line -match '^\s*\*?\s*([^\s]+)\s+([^\s]+)\s+([12])\s*$') {
            $distros += [PSCustomObject]@{
                Name = $Matches[1]; State = $Matches[2]; Version = [int]$Matches[3]
            }
        }
    }
    return $distros
}

function Get-SelectedDistro {
    return @(Get-Distros) | Where-Object { $_.Name -eq $DistroName } | Select-Object -First 1
}

function Assert-NativeUnsupportedProof([string]$Path) {
    if ([string]::IsNullOrWhiteSpace($Path)) { throw "install requires -NativeUnsupportedProof" }
    $resolved = Resolve-Path -LiteralPath $Path -ErrorAction Stop
    $proof = Get-Content -LiteralPath $resolved -Raw | ConvertFrom-Json
    if ($proof.schema -ne "govfolio.native-unsupported/v1" -or
        $proof.outcome -ne "native_unsupported" -or
        $proof.reason -ne "codex_bad_executable_format") {
        throw "native unsupported proof has an ineligible schema/outcome/reason"
    }
    if ([string]::IsNullOrWhiteSpace($proof.executable)) {
        throw "native unsupported proof has no executable"
    }
    if ($null -ne $proof.executable_sha256 -and $proof.executable_sha256 -notmatch '^[0-9a-f]{64}$') {
        throw "native unsupported proof executable hash is invalid"
    }
    $checkedAt = [DateTimeOffset]::Parse($proof.checked_at)
    $age = [DateTimeOffset]::UtcNow - $checkedAt.ToUniversalTime()
    if ($age.TotalMinutes -lt -5 -or $age.TotalHours -gt 24) {
        throw "native unsupported proof is outside the 24-hour validity window"
    }
}

function Invoke-InDistro([string]$User, [string]$Script) {
    return Invoke-Wsl @("--distribution", $DistroName, "--user", $User, "--", "sh", "-lc", $Script)
}

function Verify-Distro {
    $selected = Get-SelectedDistro
    if ($null -eq $selected) { throw "worker distro $DistroName is not installed" }
    if ($selected.Version -ne 2) { throw "worker distro $DistroName is not WSL2" }
    $userResult = Invoke-InDistro $LoopUser "id -u; id -un"
    if ($userResult.ExitCode -ne 0) { throw "loop user verification failed: $($userResult.Output)" }
    $userLines = @($userResult.Output -split "`r?`n")
    if ($userLines.Count -lt 2 -or $userLines[0].Trim() -eq "0" -or $userLines[1].Trim() -ne $LoopUser) {
        throw "loop worker must run as the non-root user $LoopUser"
    }
    $fsResult = Invoke-InDistro $LoopUser "stat -f -c %T -- '$LaneRoot'"
    if ($fsResult.ExitCode -ne 0 -or $fsResult.Output.Trim() -ne "ext2/ext3") {
        throw "lane worktree root must be on the distro ext4 filesystem"
    }
    foreach ($tool in @("git", "cargo", "rustc", "codex")) {
        $toolResult = Invoke-InDistro $LoopUser "p=`$(command -v $tool) && readlink -f -- `"`$p`""
        if ($toolResult.ExitCode -ne 0 -or [string]::IsNullOrWhiteSpace($toolResult.Output)) {
            throw "required Linux tool is unavailable: $tool"
        }
        $toolPath = $toolResult.Output.Trim().Replace('\', '/')
        if ($toolPath.StartsWith("/mnt/") -or $toolPath.EndsWith(".exe")) {
            throw "Windows interop shim is forbidden for ${tool}: $toolPath"
        }
    }
    return [PSCustomObject]@{
        status = "verified"; distro = $DistroName; version = 2; user = $LoopUser; laneRoot = $LaneRoot
    }
}

Assert-SafeName $DistroName "DistroName"
Assert-SafeName $LoopUser "LoopUser"
Assert-SafeLinuxPath $LaneRoot
Assert-WorkerDistro $DistroName
$script:ResolvedWsl = Get-WslCommand

if ($Command -eq "status") {
    if ($null -eq $script:ResolvedWsl) {
        [PSCustomObject]@{ status = "unavailable"; distro = $DistroName; installed = $false } |
            ConvertTo-Json -Compress
        exit 0
    }
    $selected = Get-SelectedDistro
    if ($null -eq $selected) {
        [PSCustomObject]@{ status = "not_installed"; distro = $DistroName; installed = $false } |
            ConvertTo-Json -Compress
        exit 0
    }
    [PSCustomObject]@{
        status = "installed"; distro = $selected.Name; state = $selected.State;
        version = $selected.Version; installed = $true
    } | ConvertTo-Json -Compress
    exit 0
}

if ($null -eq $script:ResolvedWsl) { throw "wsl.exe is unavailable" }
if ($Command -eq "verify") {
    Verify-Distro | ConvertTo-Json -Compress
    exit 0
}
if (-not $ConfirmInstall) { throw "install requires the explicit -ConfirmInstall switch" }
Assert-NativeUnsupportedProof $NativeUnsupportedProof

$selected = Get-SelectedDistro
if ($null -eq $selected) {
    $install = Invoke-Wsl @("--install", "--distribution", $DistroName, "--no-launch")
    if ($install.ExitCode -ne 0) { throw "WSL distro install failed: $($install.Output)" }
    $selected = Get-SelectedDistro
    if ($null -eq $selected) { throw "WSL install requires a reboot before verification can continue" }
}
if ($selected.Version -ne 2) {
    $convert = Invoke-Wsl @("--set-version", $DistroName, "2")
    if ($convert.ExitCode -ne 0) { throw "failed to convert worker distro to WSL2: $($convert.Output)" }
}
$createUser = Invoke-InDistro "root" "id -u '$LoopUser' >/dev/null 2>&1 || useradd -m -s /bin/sh '$LoopUser'"
if ($createUser.ExitCode -ne 0) { throw "failed to create loop user: $($createUser.Output)" }
$defaultUser = Invoke-Wsl @("--manage", $DistroName, "--set-default-user", $LoopUser)
if ($defaultUser.ExitCode -ne 0) { throw "failed to set non-root default user: $($defaultUser.Output)" }
$createRoot = Invoke-InDistro $LoopUser "mkdir -p -- '$LaneRoot'"
if ($createRoot.ExitCode -ne 0) { throw "failed to create lane root: $($createRoot.Output)" }
Verify-Distro | ConvertTo-Json -Compress

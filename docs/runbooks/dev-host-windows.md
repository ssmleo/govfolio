# Runbook: Windows dev host (no admin)

Host profile: Windows 11 Pro, standard user (**no admin rights**), no Docker, no WSL.
Rust toolchain: `stable-x86_64-pc-windows-gnu` (pinned by `rust-toolchain.toml`).
Everything below is user-scope and reversible. Machine-scope PATH/registry were never touched.

## Quirk index

| # | Quirk | State | Where |
|---|-------|-------|-------|
| 1 | cargo/rustup not on PATH in new shells | **FIXED** (persistent, user PATH) | [§1](#1-cargo-on-path) |
| 2 | Portable PG: no autostart, no client tools | **SCRIPTED** (`scripts/dev/pg-local.ps1`) + documented | [§2](#2-postgresql-1614-portable-zonky) |
| 3 | gcc absent → cc-rs "Compiler family detection failed" | **FIXED** (WinLibs via winget, user scope) | [§3](#3-gcc--c-compiling-crates) |
| 4 | PowerShell 5.1 syntax/encoding traps | **DOCUMENTED** only | [§4](#4-powershell-51-gotchas) |
| 5 | Separate worktrees repeatedly compile the same dependencies | **OPTIONAL** `sccache` wrapper | [§5](#5-optional-sccache-for-private-worktree-targets) |

## 1. cargo on PATH

`%USERPROFILE%\.cargo\bin` is on the **user** `Path` persistently (verified present and
first entry on 2026-07-04; no duplicate; machine `Path` untouched). New PowerShell and
Git Bash shells resolve `cargo`/`rustup` without any export. Shells opened before the
change need a restart (they snapshot the environment at launch) — that is what the old
`export PATH="$PATH:$HOME/.cargo/bin"` workaround was compensating for.

If it ever needs re-applying, use exactly this pattern (**never `setx`** — it truncates
at 1024 chars):

```powershell
$cur = [Environment]::GetEnvironmentVariable('Path', 'User')
if (($cur -split ';') -notcontains "$env:USERPROFILE\.cargo\bin") {
    [Environment]::SetEnvironmentVariable('Path', "$cur;$env:USERPROFILE\.cargo\bin", 'User')
}
```

## 2. PostgreSQL 16.14 (portable, zonky)

Substitutes `docker-compose.yml` locally (no Docker on this host). CI still uses the
postgres:16 service container.

| What | Where |
|------|-------|
| Binaries | `%LOCALAPPDATA%\govfolio\pg16\bin` (extracted from zonky embedded-postgres jar) |
| Data dir | `%LOCALAPPDATA%\govfolio\data` |
| Server log | `%LOCALAPPDATA%\govfolio\pg.log` |
| Port | `5433` (passed via `pg_ctl -o "-p 5433"`; `postgresql.conf` keeps defaults) |
| Auth | trust, localhost only · superuser `postgres` · db `govfolio` |
| URL | `postgres://postgres:postgres@localhost:5433/govfolio` (same as `.env.example`) |

The bundle ships **only** `initdb`/`pg_ctl`/`postgres` — no `psql`, `pg_isready`,
`createdb`, `pg_dump`.

### Lifecycle (scripted)

Does **not** auto-start after reboot. Use the repo script (verified 2026-07-04:
start/stop/status/double-stop/idempotent-start all green):

```powershell
.\scripts\dev\pg-local.ps1 start    # no-op if already running; refuses if port 5433 is foreign
.\scripts\dev\pg-local.ps1 stop     # fast shutdown
.\scripts\dev\pg-local.ps1 status   # pg_ctl status + port probe
```

### Optional: autostart on logon (NOT set up — run only if you want it)

```powershell
schtasks /Create /TN "govfolio-pg" /SC ONLOGON /RL LIMITED /F `
  /TR "powershell.exe -NoProfile -ExecutionPolicy Bypass -File C:\projects\govfolio.io\scripts\dev\pg-local.ps1 start"
# undo: schtasks /Delete /TN "govfolio-pg" /F
```

If `schtasks` denies ONLOGON for a standard user on this box, the HKCU Run key is the
guaranteed no-admin fallback:

```powershell
New-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run' -Name 'govfolio-pg' `
  -Value 'powershell.exe -NoProfile -WindowStyle Hidden -ExecutionPolicy Bypass -File C:\projects\govfolio.io\scripts\dev\pg-local.ps1 start' `
  -PropertyType String
# undo: Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run' -Name 'govfolio-pg'
```

### Living without psql

- **Interactive SQL:** the DB Toolbox MCP (`toolbox --prebuilt postgres --stdio`,
  see `docs/runbooks/deploy.md`) is wired to `localhost:5433/govfolio` — `execute_sql`
  covers everything psql would.
- **Tests:** `#[sqlx::test]` suites self-manage (create/drop their own databases);
  just have the server running: `DATABASE_URL=postgres://postgres:postgres@localhost:5433/govfolio cargo test --workspace -- --ignored`.
- **Admin ops (no server needed):** single-user mode. Stop the server first —
  single-user needs exclusive data-dir access. **Pipe SQL from Git Bash, not
  PowerShell** — on this host a PS 5.1 pipe prepends a UTF-8 BOM and the backend
  errors with `syntax error at or near "﻿SELECT"` (verified 2026-07-04). Also note the
  single-user backend exits 0 even on SQL errors — check the output, not the exit code.

  ```bash
  powershell -NoProfile -ExecutionPolicy Bypass -File scripts/dev/pg-local.ps1 stop
  echo 'CREATE DATABASE govfolio;' | "$LOCALAPPDATA/govfolio/pg16/bin/postgres.exe" --single -D "$LOCALAPPDATA/govfolio/data" postgres
  powershell -NoProfile -ExecutionPolicy Bypass -File scripts/dev/pg-local.ps1 start
  ```
- **Why psql was not installed** (fail closed): the only clean drop-in source is the
  EDB "binaries zip", which publishes no checksums, and its `psql.exe` carries its own
  libpq/DLL set that would mix with the zonky bundle's DLLs in the same `bin`. If you
  want a real psql later, extract the EDB zip to a **separate** directory (e.g.
  `%LOCALAPPDATA%\govfolio\pgclient`) and put only that `bin` on PATH — do not merge
  into `pg16\bin`.

### Recovery: rebuild the cluster from scratch

Run from Git Bash at the repo root (single-user step needs a BOM-free pipe, see above):

```bash
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/dev/pg-local.ps1 stop
rm -rf "$LOCALAPPDATA/govfolio/data"
"$LOCALAPPDATA/govfolio/pg16/bin/initdb.exe" -D "$LOCALAPPDATA/govfolio/data" -U postgres -E UTF8 -A trust
echo 'CREATE DATABASE govfolio;' | "$LOCALAPPDATA/govfolio/pg16/bin/postgres.exe" --single -D "$LOCALAPPDATA/govfolio/data" postgres
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/dev/pg-local.ps1 start
DATABASE_URL='postgres://postgres:postgres@localhost:5433/govfolio' cargo run -p core --bin migrate
```

## 3. gcc + C-compiling crates

**Installed (user scope, no admin), 2026-07-04 17:27:**

```powershell
winget install BrechtSanders.WinLibs.POSIX.UCRT   # gcc 16.1.0, mingw-w64 14.0.0 r2
```

Lives under `%LOCALAPPDATA%\Microsoft\WinGet\Packages\BrechtSanders.WinLibs.POSIX.UCRT_...\mingw64\bin`,
which is on the user `Path`. Upgrade later with `winget upgrade BrechtSanders.WinLibs.POSIX.UCRT`.

Why this is safe for the existing windows-gnu builds:

- **Linking is unchanged.** rustc ships its own self-contained mingw
  (`%USERPROFILE%\.rustup\toolchains\stable-x86_64-pc-windows-gnu\lib\rustlib\x86_64-pc-windows-gnu\bin\self-contained\`
  — `ld.exe`, `x86_64-w64-mingw32-gcc.exe`) and keeps preferring it; PATH gcc does not
  hijack the link step.
- **cc-rs now finds a real compiler.** `ring` (via reqwest/rustls/sqlx) compiles C
  through cc-rs; before this install it emitted the nonfatal
  `Compiler family detection failed ... gcc.exe not found` warning and would have
  hard-failed on any dep that truly needs a full gcc (pdfium-style).

**Verified 2026-07-04 ~20:00** (after forcing a fresh ring rebuild with `cargo clean -p ring`):

- `cargo clippy --all-targets -- -D warnings` → green, **zero** compiler-detection warnings
- `cargo test --workspace` → green (42 passed)
- `cargo test --workspace -- --ignored` against local PG → green (4 passed)

Legacy note: `~/tools/mingw64` (a manual mingw copy that used to be prepended to PATH
per `agents/skills/rust-tdd/SKILL.md`) is **superseded** by the WinLibs install. It is
not on the persistent PATH and can be deleted once nothing references it.

## 4. PowerShell 5.1 gotchas (documented only)

Default shell is Windows PowerShell 5.1, not pwsh 7:

- No `&&` / `||` pipeline chains — use `A; if ($?) { B }`.
- No ternary `?:`, no `??`, no `?.`.
- `Out-File` / `Set-Content` / `>` default to **UTF-16 LE** — pass `-Encoding utf8`
  when other tools will read the file.
- **`.ps1` files without a BOM are parsed as ANSI.** UTF-8 multibyte chars decode to
  cp1252 garbage; notably the em-dash (`E2 80 94`) ends in `0x94` = a *smart quote*,
  which PS 5.1 treats as a string terminator → phantom parse errors far from the real
  cause (hit this with `pg-local.ps1` on 2026-07-04). Keep repo `.ps1` files pure
  ASCII, or save UTF-8 *with* BOM.
- Native-exe `2>&1` inside PS 5.1 wraps stderr lines in ErrorRecords and flips `$?`
  even on exit 0 — avoid redirecting native stderr.
- Piping a string into a native exe's stdin prepends a UTF-8 BOM on this host (even
  with `$OutputEncoding = ASCII`) — byte-exact stdin must go through Git Bash or an
  ASCII file redirect.

## 5. Optional sccache for private worktree targets

When `sccache` is installed and available on `Path`, use the repository wrapper instead
of invoking Cargo directly:

```powershell
.\scripts\dev\cargo-agent.ps1 test --workspace
.\scripts\dev\cargo-agent.ps1 clippy --all-targets -- -D warnings
```

The wrapper stores compiler-cache entries under
`%LOCALAPPDATA%\govfolio\sccache`, sets `CARGO_INCREMENTAL=0` so eligible dependency
compilations can be cached, and prints cache statistics after Cargo exits. The wrapper
returns Cargo's exit code. It intentionally does not set `CARGO_TARGET_DIR`: Cargo's
default remains the current worktree's private `target` directory, and an explicitly
configured private target is preserved. Never configure one mutable Cargo target for
concurrent worktrees. Registry data and `sccache` entries are safe to share; target
artifacts and their build locks are not.

The wrapper is optional, but its behavior is fail-closed: if `sccache` is absent it
exits with a clear error instead of silently running an uncached build. Invoke `cargo`
directly when an uncached build is intended. Cache failure cannot make a stale artifact
pass because rustc remains responsible for every cache key and Cargo still owns the
worktree-private dependency graph.

### Move future loop Bronze writes outside `target`

`agents/run-loop.sh` now defaults `GOVFOLIO_BRONZE_ROOT` to the absolute sibling
directory `<repository-parent>\govfolio-bronze`. An explicit absolute environment value
still takes precedence. This changes future loop writes only; it does not migrate or
delete Bronze already stored below a checkout's `target` directory.

Before switching a host that has existing `target\bronze-*` stores, stop loop writers
and make a copy-only migration. The following PowerShell 5.1-compatible procedure copies
each store without `/MOVE` or `/MIR`, then compares relative path, byte length, and
SHA-256 for every file:

```powershell
$repo = (Resolve-Path '.').Path
$oldParent = Join-Path $repo 'target'
$newParent = [IO.Path]::GetFullPath((Join-Path $repo '..\govfolio-bronze'))
$stores = @(Get-ChildItem -LiteralPath $oldParent -Directory -Filter 'bronze-*')
if ($stores.Count -eq 0) {
    throw "No Bronze stores found below $oldParent; verify the source worktree"
}

New-Item -ItemType Directory -Force -Path $newParent | Out-Null
foreach ($store in $stores) {
    $destination = Join-Path $newParent $store.Name
    & robocopy.exe $store.FullName $destination /E /COPY:DAT /DCOPY:DAT /R:1 /W:1
    if ($LASTEXITCODE -gt 7) {
        throw "Bronze copy failed for $($store.FullName): robocopy exit $LASTEXITCODE"
    }

    $sourcePrefix = $store.FullName.TrimEnd('\') + '\'
    $sourceFiles = @(Get-ChildItem -LiteralPath $store.FullName -File -Recurse)
    $verified = 0
    foreach ($sourceFile in $sourceFiles) {
        $relative = $sourceFile.FullName.Substring($sourcePrefix.Length)
        $destinationFile = Join-Path $destination $relative
        if (-not (Test-Path -LiteralPath $destinationFile -PathType Leaf)) {
            throw "Missing copied Bronze file: $destinationFile"
        }
        $destinationLength = (Get-Item -LiteralPath $destinationFile).Length
        $sourceHash = (Get-FileHash -LiteralPath $sourceFile.FullName -Algorithm SHA256).Hash
        $destinationHash = (Get-FileHash -LiteralPath $destinationFile -Algorithm SHA256).Hash
        if ($sourceFile.Length -ne $destinationLength -or $sourceHash -ne $destinationHash) {
            throw "Bronze verification failed for $relative; keep both copies and investigate"
        }
        $verified += 1
    }
    if ($verified -ne $sourceFiles.Count) {
        throw "Bronze file-count verification failed for $($store.Name)"
    }
    Write-Host "Verified $($store.Name): $verified source files"
}
```

Keep the old `target\bronze-*` trees after verification and run the loop against the new
root before considering any retention decision. Repeat the copy-and-verify pass from
each old worktree; the shared destination may safely accumulate additional verified
content-addressed files. Do not run `cargo clean`, delete an old target, or remove old
Bronze as part of this migration. Raw documents are sacred; any missing store, count
mismatch, hash mismatch, or concurrent writer is a hard stop.

## Audit trail (user-environment changes, all reversible)

| When (local) | Change | Scope | Undo |
|--------------|--------|-------|------|
| 2026-07-04 (≤17:27 session) | `%USERPROFILE%\.cargo\bin` appended to user `Path` | HKCU env | remove entry from user `Path` |
| 2026-07-04 17:27 | WinLibs gcc via winget + its `mingw64\bin` on user `Path` | `%LOCALAPPDATA%` + HKCU env | `winget uninstall BrechtSanders.WinLibs.POSIX.UCRT` + remove `Path` entry |
| 2026-07-04 20:05 | `scripts/dev/pg-local.ps1` added (repo only; no host change) | repo | git |
| — | Autostart task **not** created; machine PATH **not** touched; nothing installed outside `%LOCALAPPDATA%`/`%USERPROFILE%` | — | — |

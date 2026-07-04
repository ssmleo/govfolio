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

## Audit trail (user-environment changes, all reversible)

| When (local) | Change | Scope | Undo |
|--------------|--------|-------|------|
| 2026-07-04 (≤17:27 session) | `%USERPROFILE%\.cargo\bin` appended to user `Path` | HKCU env | remove entry from user `Path` |
| 2026-07-04 17:27 | WinLibs gcc via winget + its `mingw64\bin` on user `Path` | `%LOCALAPPDATA%` + HKCU env | `winget uninstall BrechtSanders.WinLibs.POSIX.UCRT` + remove `Path` entry |
| 2026-07-04 20:05 | `scripts/dev/pg-local.ps1` added (repo only; no host change) | repo | git |
| — | Autostart task **not** created; machine PATH **not** touched; nothing installed outside `%LOCALAPPDATA%`/`%USERPROFILE%` | — | — |

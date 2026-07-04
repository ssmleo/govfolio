# skill: rust-tdd
Purpose: red -> green -> commit under the lint law
Load when: BUILD phase, any crate change
Core checklist:
- failing test first -> minimal impl -> cargo fmt/clippy -D warnings/test workspace -> commit small
Anti-patterns: unwrap outside tests; skipping red; broad commits
Learnings (dated):
- 2026-07-04: package `core` keeps its name, but its lib target is `govfolio_core` — an --extern named `core` shadows sysroot core inside the package's own tests/bins and breaks proc macros emitting `::core::...` (#[tokio::main], #[sqlx::test]). Import as `govfolio_core::...`; `-p core` commands unchanged.
- 2026-07-04: this Windows host is x86_64-pc-windows-gnu without MSVC; builds with C deps (ring) need `~/tools/mingw64/bin` prepended to PATH (rustup self-contained gcc is linker-only, its dlltool lacks `as`). DB suites gate as `#[sqlx::test(migrations = false)]` + `#[ignore = "needs postgres"]`; local server: portable pg16 on 5433, trust auth.
Write-back: deepen this file when the procedure teaches you something; same PR.

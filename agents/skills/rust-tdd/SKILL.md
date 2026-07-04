# skill: rust-tdd
Purpose: red -> green -> commit under the lint law
Load when: BUILD phase, any crate change
Core checklist:
- failing test first -> minimal impl -> cargo fmt/clippy -D warnings/test workspace -> commit small
Anti-patterns: unwrap outside tests; skipping red; broad commits
Learnings (dated):
- 2026-07-04: package `core` keeps its name, but its lib target is `govfolio_core` — an --extern named `core` shadows sysroot core inside the package's own tests/bins and breaks proc macros emitting `::core::...` (#[tokio::main], #[sqlx::test]). Import as `govfolio_core::...`; `-p core` commands unchanged.
- 2026-07-04: this Windows host is x86_64-pc-windows-gnu without MSVC; builds with C deps (ring) need `~/tools/mingw64/bin` prepended to PATH (rustup self-contained gcc is linker-only, its dlltool lacks `as`). DB suites gate as `#[sqlx::test(migrations = false)]` + `#[ignore = "needs postgres"]`; local server: portable pg16 on 5433, trust auth.
- 2026-07-04: sqlx 0.8→0.9.0 is drop-in for our feature set (runtime-tokio, tls-rustls, postgres, migrate, macros; `tls-rustls` now resolves to ring — good for this gnu host) EXCEPT `raw_sql`/`query` with a built String: new `SqlSafeStr` bound rejects dynamic SQL; wrap test-constant SQL in `sqlx::AssertSqlSafe(..)`.
- 2026-07-04: `sqlx::migrate!` embeds migration files at compile time — adding a new .sql does NOT rebuild the lib, so the embedded migrator silently runs stale (42P01 on brand-new tables while migrate() "succeeds"). Fix once per crate: build.rs with `cargo:rerun-if-changed=migrations`.
- 2026-07-04: hashing canonical JSON — don't rely on `serde_json::Map`'s default BTreeMap backing for key order: any crate enabling `preserve_order` flips it workspace-wide via feature unification. Sort keys explicitly. `Value`'s `Display` (`.to_string()`) gives compact serialization with NO `Result` — sidesteps the unwrap ban that `serde_json::to_string` would force into infallible paths.
- 2026-07-04: workspace `exclude = ["crates/adapters"]` also drops every package
  BENEATH it even when a members glob matches them (globs lose to exclude; explicitly
  listed member paths win), while a bare `crates/*` glob errors on the package-less
  adapters dir. Working shape: explicit top-level members + `"crates/adapters/*"`
  glob, no exclude.
- 2026-07-04: keep aws-lc-rs out of the dep graph on this gnu host (wants cmake/NASM):
  reqwest 0.13's `rustls` feature hard-binds aws-lc-rs — use `rustls-no-provider`
  plus a direct rustls dep with the `ring` feature and
  `CryptoProvider::install_default(ring)` at client build; jsonschema's default
  features pull reqwest (remote $ref resolving) — `default-features = false` suffices
  for local schema docs. `tokio::time::pause`/`start_paused` needs the `test-util`
  feature (dev-dependency).
Write-back: deepen this file when the procedure teaches you something; same PR.

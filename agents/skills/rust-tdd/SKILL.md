# skill: rust-tdd
Purpose: red -> green -> commit under the lint law
Load when: BUILD phase, any crate change
Core checklist:
- failing test first -> minimal impl -> cargo fmt/clippy -D warnings/test workspace -> commit small
Anti-patterns: unwrap outside tests; skipping red; broad commits
Discipline deltas (distilled 2026-07-05 from imported/superpowers@d884ae04edeb/test-driven-development, goal 019; this bespoke file stays authoritative):
- Verify RED for the RIGHT reason: the test must fail because the feature is missing — not a typo/compile error. Test passes immediately = it tests existing behavior; fix the test, not the code.
- Wrote impl before the test? Delete it and re-derive from the test — no "keep as reference", no adapting; adapted code is tests-after with extra steps.
- Regression tests need a red-green proof: pass -> revert fix -> MUST fail -> restore -> pass. A test that never failed proves nothing.
- Green means pristine: whole suite passes AND output clean (no new warnings) — matches our `-D warnings` law.
- Hard-to-test is a design signal: huge setup or mock-everything means the interface is wrong — simplify the interface, don't grow the test harness. Write the wished-for API/assertion first when stuck.
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
- 2026-07-04: constants under the unwrap ban — `NaiveDate::from_ymd_opt` is const-evaluable:
  `const D: NaiveDate = match NaiveDate::from_ymd_opt(..) { Some(d) => d, None => panic!("..") };`
  proves the date at compile time (const panic is not clippy::unwrap_used/expect_used).
  Related pedantic trap: `similar_names` denies close bindings (`stats`/`status`, `args`/`argv`).
- 2026-07-04: `#[derive(sqlx::FromRow)]` maps INT4→u32 fields via `#[sqlx(try_from = "i32")]`;
  a non-core crate can host `#[sqlx::test]` suites with dev-dep `sqlx { features = ["macros",
  "migrate", ...] }` (pattern: pipeline's e2e drives us_house via the legal dev-dep cycle).
- 2026-07-04: sqlx 0.9 `SqlSafeStr` in production paths without `AssertSqlSafe`: when
  several queries share one projection, build them with `macro_rules!` + `concat!`
  (`record_select!("where ... limit $4")`) — stays a compile-time `&'static str`, so the
  injection guarantee holds structurally. Static SQL + `($n::text is null or col = $n)`
  binds covers optional filters without dynamic SQL.
- 2026-07-04: proving row immutability (invariant 1 supersession tests): `select d::text
  from disclosure_record d where id = $1` renders the WHOLE row as one Postgres composite
  literal — a before/after string compare is a byte-level all-columns probe with zero
  column-list drift risk; `(to_jsonb(d) - 'verification_state')::text` gives the
  all-but-one-column variant for sanctioned single-column transitions.
- 2026-07-05: serde_json f32 literals take TWO shapes for the SAME value: `to_value`/`Value`
  routes f32 through an f64 cast (0.9f32 → `0.8999999761581421`) while direct struct
  serialization uses ryu shortest-form (`0.9`). Both parse back to the identical f32 (with
  `float_roundtrip` on), so committed artifacts may legitimately differ textually from
  expected.*.json literals — compare in VALUE space (parse both sides), never bytes.
  Related: `clippy::float_cmp` fires on exact-f32 contract assertions — scope an
  `allow(clippy::float_cmp)` to the test mod with a comment saying bit-equality IS the contract.
- 2026-07-05: mock seams over HTTP — a `#[async_trait] trait Transport { send(&Value) -> Value }`
  plus a blanket `impl Transport for &T` lets prod code take `T: Transport` by value while
  callers keep ownership (tests pass `&MockTransport` and can inspect recorded requests after).
  Retry/backoff factored as a free `with_backoff(max_retries, base, op)` unit-tests cleanly under
  `#[tokio::test(start_paused = true)]` (paused clock auto-advances sleeps — asserting elapsed
  backoff time costs zero wall-clock).
- 2026-07-05: `cargo test --workspace -- --ignored` RUNS ignored tests — an
  `#[ignore = "needs SECRET"]` live test must ALSO early-return (loudly) when the env var is
  absent, or the offline gate goes red on hosts without the secret.
Write-back: deepen this file when the procedure teaches you something; same PR.

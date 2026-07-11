//! `cargo run -p pipeline --bin validate-authority [-- --ci | --write-lock
//! [--note <text>] | --check-path <p>]` — invariant 9 made mechanical
//! (goal 100, design §4.2): goals↔000-INDEX bijection, `AUTHORITY.lock.json`
//! sha256 pins, `--ci` amendment discipline, and the hook's fast single-path
//! verdict (deny → exit 2). Fail closed on any ambiguity.

fn main() -> std::process::ExitCode {
    pipeline::factory::authority_bin_main()
}

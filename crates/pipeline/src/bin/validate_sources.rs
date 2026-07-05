//! `cargo run -p pipeline --bin validate-sources -- <jurisdiction>` — gates
//! the Scout artifact `docs/regimes/<x>/sources.yaml` (workflow Phase 0).

fn main() -> std::process::ExitCode {
    pipeline::factory::bin_main(pipeline::factory::Gate::Sources)
}

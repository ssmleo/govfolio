//! `cargo run -p pipeline --bin validate-manifest -- <jurisdiction>` — gates
//! the Sampler artifact: `crates/adapters/<x>/fixtures/*/input.*` plus the
//! capture manifest (workflow Phase 2).

fn main() -> std::process::ExitCode {
    pipeline::factory::bin_main(pipeline::factory::Gate::Manifest)
}

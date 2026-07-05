//! `cargo run -p pipeline --bin validate-survey -- <jurisdiction>` — gates
//! the Surveyor artifact: `docs/regimes/<x>/AUTHORITY.md` front-matter against
//! the `RegimeSurvey` schema (workflow Phase 1).

fn main() -> std::process::ExitCode {
    pipeline::factory::bin_main(pipeline::factory::Gate::Survey)
}

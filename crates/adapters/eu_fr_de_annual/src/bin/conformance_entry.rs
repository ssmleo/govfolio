//! Conformance entry for `eu_fr_de_annual`, dispatched by
//! `cargo run -p pipeline --bin conformance -- eu_fr_de_annual`.

use std::process::ExitCode;

fn main() -> ExitCode {
    pipeline::conformance::adapter_entry(
        &eu_fr_de_annual::EuFrDeAnnualAdapter::default(),
        "eu_fr_de_annual",
    )
}

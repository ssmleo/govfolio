//! Conformance entry for `canada_ciec`, dispatched by
//! `cargo run -p pipeline --bin conformance -- canada_ciec`.

use std::process::ExitCode;

fn main() -> ExitCode {
    pipeline::conformance::adapter_entry(&canada_ciec::CanadaCiecAdapter, "canada_ciec")
}

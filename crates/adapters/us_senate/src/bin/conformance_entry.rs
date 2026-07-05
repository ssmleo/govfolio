//! Conformance entry for `us_senate`, dispatched by
//! `cargo run -p pipeline --bin conformance -- us_senate`.

use std::process::ExitCode;

fn main() -> ExitCode {
    pipeline::conformance::adapter_entry(&us_senate::UsSenateAdapter::default(), "us_senate")
}

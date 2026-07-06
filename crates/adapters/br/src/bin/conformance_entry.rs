//! Conformance entry for `br`, dispatched by
//! `cargo run -p pipeline --bin conformance -- br`.

use std::process::ExitCode;

fn main() -> ExitCode {
    pipeline::conformance::adapter_entry(&br::BrAdapter::default(), "br")
}

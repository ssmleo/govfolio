//! Conformance entry for `australia_register`, dispatched by
//! `cargo run -p pipeline --bin conformance -- australia_register`.

use std::process::ExitCode;

fn main() -> ExitCode {
    pipeline::conformance::adapter_entry(
        &australia_register::AustraliaRegisterAdapter::default(),
        "australia_register",
    )
}

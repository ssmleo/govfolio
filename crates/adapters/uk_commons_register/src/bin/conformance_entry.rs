//! Conformance entry for `uk_commons_register`, dispatched by
//! `cargo run -p pipeline --bin conformance -- uk_commons_register`.

use std::process::ExitCode;

fn main() -> ExitCode {
    pipeline::conformance::adapter_entry(
        &uk_commons_register::UkCommonsRegisterAdapter,
        "uk_commons_register",
    )
}

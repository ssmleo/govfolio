//! Conformance entry for `fixture_fake`, dispatched by
//! `cargo run -p pipeline --bin conformance -- fixture_fake`.

use std::process::ExitCode;

fn main() -> ExitCode {
    pipeline::conformance::adapter_entry(&fixture_fake::FixtureFakeAdapter, "fixture_fake")
}

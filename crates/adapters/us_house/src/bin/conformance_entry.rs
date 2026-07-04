//! Conformance entry for `us_house`, dispatched by
//! `cargo run -p pipeline --bin conformance -- us_house`.

use std::process::ExitCode;

fn main() -> ExitCode {
    pipeline::conformance::adapter_entry(&us_house::UsHouseAdapter::default(), "us_house")
}

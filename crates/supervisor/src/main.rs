fn main() -> std::process::ExitCode {
    match loop_supervisor::supervisor::cli_main() {
        Ok(code) => std::process::ExitCode::from(code),
        Err(error) => {
            eprintln!("govfolio-loop: {error:#}");
            std::process::ExitCode::FAILURE
        }
    }
}

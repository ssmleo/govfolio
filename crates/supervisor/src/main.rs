fn main() -> std::process::ExitCode {
    match loop_supervisor::supervisor::cli_main() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("govfolio-loop: {error:#}");
            std::process::ExitCode::FAILURE
        }
    }
}

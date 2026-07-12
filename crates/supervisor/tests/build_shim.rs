#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::process::Command;

use loop_supervisor::build_shim::{install_cargo_shim, prepend_path, resolve_real_cargo};

#[test]
fn build_shim_is_immutable_idempotent_and_precedes_real_cargo() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp
        .path()
        .join(if cfg!(windows) { "loop.exe" } else { "loop" });
    std::fs::write(&source, b"immutable-supervisor").unwrap();

    let first = install_cargo_shim(temp.path(), &source).unwrap();
    let second = install_cargo_shim(temp.path(), &source).unwrap();
    assert_eq!(first, second);
    assert_eq!(
        std::fs::read(&first.executable).unwrap(),
        b"immutable-supervisor"
    );
    assert_eq!(first.executable.file_stem().unwrap(), "cargo");

    let existing = std::env::var_os("PATH").unwrap();
    let real = resolve_real_cargo(&existing, &temp.path().join("build-shims")).unwrap();
    assert_ne!(real, first.executable);
    let path = prepend_path(&first.path_entry, &existing).unwrap();
    assert_eq!(
        std::env::split_paths(std::ffi::OsStr::new(&path))
            .next()
            .unwrap(),
        first.path_entry
    );
}

#[test]
fn copied_supervisor_binary_dispatches_as_cargo() {
    let temp = tempfile::tempdir().unwrap();
    let source = env!("CARGO_BIN_EXE_govfolio-loop");
    let shim = install_cargo_shim(temp.path(), std::path::Path::new(source)).unwrap();
    let output = Command::new(&shim.executable)
        .arg("--version")
        .env("CARGO_TERM_COLOR", "never")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).starts_with("cargo "),
        "stdout={:?}; stderr={:?}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[cfg(unix)]
#[test]
fn real_cargo_resolution_preserves_a_rustup_proxy_invocation_name() {
    use std::os::unix::fs::symlink;

    let temp = tempfile::tempdir().unwrap();
    let bin = temp.path().join("bin");
    std::fs::create_dir(&bin).unwrap();
    let rustup = bin.join("rustup");
    std::fs::write(&rustup, b"rustup proxy").unwrap();
    symlink(&rustup, bin.join("cargo")).unwrap();

    let resolved = resolve_real_cargo(bin.as_os_str(), &temp.path().join("forbidden")).unwrap();

    assert_eq!(resolved, bin.canonicalize().unwrap().join("cargo"));
    assert_ne!(resolved, rustup.canonicalize().unwrap());
}

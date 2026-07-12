use std::ffi::OsStr;
use std::fs::OpenOptions;
use std::io::{self, Read as _, Write as _};
use std::path::{Path, PathBuf};

use anyhow::{Context as _, bail};
use sha2::{Digest as _, Sha256};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CargoShim {
    pub executable: PathBuf,
    pub path_entry: PathBuf,
    pub source_sha256: String,
}

pub fn install_cargo_shim(state_root: &Path, loop_binary: &Path) -> anyhow::Result<CargoShim> {
    let metadata = std::fs::symlink_metadata(loop_binary)
        .with_context(|| format!("inspect supervisor binary {}", loop_binary.display()))?;
    if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
        bail!(
            "supervisor binary must be a regular non-symlinked file: {}",
            loop_binary.display()
        );
    }
    let source_sha256 = file_sha256(loop_binary)?;
    let path_entry = state_root.join("build-shims").join(&source_sha256[..16]);
    std::fs::create_dir_all(&path_entry)?;
    let executable = path_entry.join(if cfg!(windows) { "cargo.exe" } else { "cargo" });
    match copy_new(loop_binary, &executable) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            if file_sha256(&executable)? != source_sha256 {
                bail!(
                    "existing Cargo shim does not match its immutable source hash: {}",
                    executable.display()
                );
            }
        }
        Err(error) => return Err(error.into()),
    }
    #[cfg(unix)]
    set_executable_permissions(&executable)?;
    Ok(CargoShim {
        executable,
        path_entry,
        source_sha256,
    })
}

pub fn resolve_real_cargo(path: &OsStr, forbidden_root: &Path) -> anyhow::Result<PathBuf> {
    for directory in std::env::split_paths(path) {
        let candidate = directory.join(if cfg!(windows) { "cargo.exe" } else { "cargo" });
        if candidate.is_file() && !candidate.starts_with(forbidden_root) {
            return candidate
                .canonicalize()
                .with_context(|| format!("canonicalize real Cargo {}", candidate.display()));
        }
    }
    bail!("real Cargo executable was not found before installing the supervisor shim")
}

pub fn prepend_path(entry: &Path, existing: &OsStr) -> anyhow::Result<String> {
    let mut paths = vec![entry.to_path_buf()];
    paths.extend(std::env::split_paths(existing));
    std::env::join_paths(paths)
        .map(|value| value.to_string_lossy().into_owned())
        .context("compose provider PATH with Cargo shim")
}

fn copy_new(source: &Path, destination: &Path) -> io::Result<()> {
    let mut source = OpenOptions::new().read(true).open(source)?;
    let mut destination = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(destination)?;
    io::copy(&mut source, &mut destination)?;
    destination.flush()?;
    destination.sync_all()
}

fn file_sha256(path: &Path) -> io::Result<String> {
    let mut file = OpenOptions::new().read(true).open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex::encode(hasher.finalize()))
}

#[cfg(unix)]
fn set_executable_permissions(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt as _;

    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
}

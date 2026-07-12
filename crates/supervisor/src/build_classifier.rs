use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceClass {
    Focused,
    Exclusive,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CargoDisposition {
    Passthrough,
    Managed(ResourceClass),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClassificationContext {
    pub worktree: PathBuf,
    pub target_dir: PathBuf,
    pub shared_target: PathBuf,
    pub bronze_roots: Vec<PathBuf>,
    pub category: Option<String>,
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum ClassificationError {
    #[error("Cargo arguments are empty")]
    Empty,
    #[error("denied Cargo command: {0}")]
    Denied(String),
    #[error("explicit focused class would downgrade an exclusive command")]
    Downgrade,
    #[error("invalid Cargo jobs value {0:?}")]
    InvalidJobs(String),
    #[error("job budget must be positive")]
    InvalidBudget,
}

pub fn classify_cargo(
    args: &[String],
    context: &ClassificationContext,
    explicit: Option<ResourceClass>,
) -> Result<CargoDisposition, ClassificationError> {
    let command = args
        .first()
        .ok_or(ClassificationError::Empty)?
        .to_ascii_lowercase();
    if command == "clean" {
        return Err(ClassificationError::Denied(
            "cargo clean is forbidden".to_owned(),
        ));
    }
    validate_storage_paths(args, context)?;
    if matches!(
        command.as_str(),
        "--version" | "version" | "metadata" | "tree" | "fmt"
    ) {
        return Ok(CargoDisposition::Passthrough);
    }

    let packages = explicit_package_count(args)?;
    let forced_exclusive = packages != 1
        || context.category.as_ref().is_some_and(|category| {
            matches!(
                category.trim().to_ascii_lowercase().as_str(),
                "cold" | "cold-build" | "benchmark" | "complete-matrix" | "epoch-gate"
            )
        })
        || has_any(
            args,
            &[
                "--workspace",
                "--all",
                "--all-packages",
                "--all-targets",
                "--all-features",
                "--benches",
                "--bench",
            ],
        )
        || (command == "test" && has_any(args, &["--no-run"]))
        || matches!(command.as_str(), "bench" | "epoch-gate");
    let inferred = if !forced_exclusive
        && matches!(
            command.as_str(),
            "check" | "build" | "test" | "clippy" | "run"
        ) {
        ResourceClass::Focused
    } else {
        ResourceClass::Exclusive
    };
    let effective = match (inferred, explicit) {
        (ResourceClass::Exclusive, Some(ResourceClass::Focused)) => {
            return Err(ClassificationError::Downgrade);
        }
        (ResourceClass::Focused, Some(ResourceClass::Exclusive)) => ResourceClass::Exclusive,
        (_, Some(class)) => class,
        (_, None) => inferred,
    };
    Ok(CargoDisposition::Managed(effective))
}

pub fn apply_job_budget(
    args: &[String],
    budget: usize,
) -> Result<Vec<String>, ClassificationError> {
    if budget == 0 {
        return Err(ClassificationError::InvalidBudget);
    }
    let mut output = Vec::with_capacity(args.len() + 2);
    let mut requested = None;
    let mut index = 0;
    while index < args.len() {
        let arg = &args[index];
        let inline = arg
            .strip_prefix("--jobs=")
            .or_else(|| arg.strip_prefix("-j").filter(|value| !value.is_empty()));
        if let Some(value) = inline {
            requested = Some(requested_jobs(value, requested)?);
            index += 1;
        } else if arg == "--jobs" || arg == "-j" {
            let value = args
                .get(index + 1)
                .ok_or_else(|| ClassificationError::InvalidJobs(arg.clone()))?;
            requested = Some(requested_jobs(value, requested)?);
            index += 2;
        } else {
            output.push(arg.clone());
            index += 1;
        }
    }
    let effective = requested.map_or(budget, |jobs| jobs.min(budget));
    output.push("--jobs".to_owned());
    output.push(effective.to_string());
    Ok(output)
}

fn requested_jobs(value: &str, previous: Option<usize>) -> Result<usize, ClassificationError> {
    let parsed = value
        .parse::<usize>()
        .ok()
        .filter(|jobs| *jobs > 0)
        .ok_or_else(|| ClassificationError::InvalidJobs(value.to_owned()))?;
    Ok(previous.map_or(parsed, |jobs| jobs.min(parsed)))
}

fn explicit_package_count(args: &[String]) -> Result<usize, ClassificationError> {
    let mut count = 0;
    let mut index = 1;
    while index < args.len() {
        let arg = &args[index];
        if arg == "-p" || arg == "--package" {
            args.get(index + 1)
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| {
                    ClassificationError::Denied("package value is missing".to_owned())
                })?;
            count += 1;
            index += 2;
        } else if arg.starts_with("--package=") {
            if arg.trim_start_matches("--package=").trim().is_empty() {
                return Err(ClassificationError::Denied(
                    "package value is missing".to_owned(),
                ));
            }
            count += 1;
            index += 1;
        } else {
            index += 1;
        }
    }
    Ok(count)
}

fn has_any(args: &[String], needles: &[&str]) -> bool {
    args.iter().any(|arg| {
        needles
            .iter()
            .any(|needle| arg == needle || arg.starts_with(&format!("{needle}=")))
    })
}

fn validate_storage_paths(
    args: &[String],
    context: &ClassificationContext,
) -> Result<(), ClassificationError> {
    if same_path(&context.target_dir, &context.shared_target) {
        return Err(ClassificationError::Denied(
            "shared mutable target directory is forbidden".to_owned(),
        ));
    }
    if same_path(&context.target_dir, &context.worktree) {
        return Err(ClassificationError::Denied(
            "worktree root cannot be used as a build target".to_owned(),
        ));
    }
    for bronze in &context.bronze_roots {
        if path_is_within(&context.target_dir, bronze) {
            return Err(ClassificationError::Denied(
                "Bronze paths are forbidden for build targets".to_owned(),
            ));
        }
    }
    for arg in args {
        if arg == "--config" || (arg.starts_with("--config=") && arg.contains("target-dir")) {
            return Err(ClassificationError::Denied(
                "Cargo config target overrides are forbidden".to_owned(),
            ));
        }
        let candidate = PathBuf::from(arg.trim_start_matches("--target-dir="));
        for bronze in &context.bronze_roots {
            if path_is_within(&candidate, bronze) {
                return Err(ClassificationError::Denied(
                    "Bronze paths are forbidden in Cargo commands".to_owned(),
                ));
            }
        }
    }
    let mut index = 0;
    while index < args.len() {
        let explicit = if args[index] == "--target-dir" {
            index += 1;
            args.get(index).map(String::as_str)
        } else {
            args[index].strip_prefix("--target-dir=")
        };
        if let Some(explicit) = explicit {
            let explicit = PathBuf::from(explicit);
            let explicit = if explicit.is_absolute() {
                explicit
            } else {
                context.worktree.join(explicit)
            };
            if !same_path(&explicit, &context.target_dir) {
                return Err(ClassificationError::Denied(
                    "Cargo target override does not match the admitted private target".to_owned(),
                ));
            }
        }
        index += 1;
    }
    Ok(())
}

fn same_path(left: &Path, right: &Path) -> bool {
    normalized(left) == normalized(right)
}

fn path_is_within(candidate: &Path, root: &Path) -> bool {
    let candidate = normalized(candidate);
    let root = normalized(root);
    candidate == root || candidate.starts_with(&format!("{root}/"))
}

fn normalized(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "/")
        .trim_end_matches('/')
        .to_ascii_lowercase()
}

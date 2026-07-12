use std::collections::BTreeMap;
use std::io;
use std::path::Path;
use std::time::Duration;

use thiserror::Error;

use crate::build_classifier::ResourceClass;

const GIB: u64 = 1024 * 1024 * 1024;
const CONFIG_KEYS: [&str; 13] = [
    "GOVFOLIO_CARGO_FOCUSED_CAPACITY",
    "GOVFOLIO_CARGO_FOCUSED_JOBS",
    "GOVFOLIO_CARGO_EXCLUSIVE_JOBS",
    "GOVFOLIO_CARGO_QUEUE_DEADLINE_SECONDS",
    "GOVFOLIO_CARGO_EXPERIMENT_DEADLINE_SECONDS",
    "GOVFOLIO_CARGO_HEARTBEAT_SECONDS",
    "GOVFOLIO_CARGO_PROGRESS_REPORT_SECONDS",
    "GOVFOLIO_CARGO_NO_PROGRESS_SECONDS",
    "GOVFOLIO_CARGO_FOCUSED_MEMORY_GIB",
    "GOVFOLIO_CARGO_EXCLUSIVE_MEMORY_GIB",
    "GOVFOLIO_CARGO_MIN_FREE_DISK_GIB",
    "GOVFOLIO_CARGO_MIN_FREE_DISK_PERCENT",
    "GOVFOLIO_CARGO_BIN",
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BuildAdmissionConfig {
    pub logical_cpus: usize,
    pub focused_capacity: usize,
    pub focused_jobs: usize,
    pub exclusive_jobs: usize,
    pub queue_deadline: Duration,
    pub experiment_deadline: Duration,
    pub heartbeat: Duration,
    pub progress_report: Duration,
    pub no_progress_deadline: Duration,
    pub focused_memory_bytes: u64,
    pub exclusive_memory_bytes: u64,
    pub minimum_free_disk_bytes: u64,
    pub minimum_free_disk_percent: u8,
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum BuildConfigError {
    #[error("logical CPU count must exceed the two reserved host CPUs")]
    InsufficientCpus,
    #[error("invalid {key} value {value:?}")]
    InvalidValue { key: String, value: String },
    #[error("unknown Cargo admission configuration {0}")]
    UnknownKey(String),
    #[error("focused or exclusive jobs consume the two reserved host CPUs")]
    CpuBudget,
}

impl BuildAdmissionConfig {
    pub fn from_env(logical_cpus: usize) -> Result<Self, BuildConfigError> {
        let values = std::env::vars()
            .filter(|(key, _)| key.starts_with("GOVFOLIO_CARGO_"))
            .collect::<BTreeMap<_, _>>();
        Self::from_values(logical_cpus, &values)
    }

    pub fn from_values(
        logical_cpus: usize,
        values: &BTreeMap<String, String>,
    ) -> Result<Self, BuildConfigError> {
        if logical_cpus <= 2 {
            return Err(BuildConfigError::InsufficientCpus);
        }
        if let Some(key) = values
            .keys()
            .find(|key| !CONFIG_KEYS.contains(&key.as_str()))
        {
            return Err(BuildConfigError::UnknownKey(key.clone()));
        }
        let config = Self {
            logical_cpus,
            focused_capacity: value(values, "GOVFOLIO_CARGO_FOCUSED_CAPACITY", 2)?,
            focused_jobs: value(values, "GOVFOLIO_CARGO_FOCUSED_JOBS", 6)?,
            exclusive_jobs: value(values, "GOVFOLIO_CARGO_EXCLUSIVE_JOBS", 14)?,
            queue_deadline: seconds(values, "GOVFOLIO_CARGO_QUEUE_DEADLINE_SECONDS", 30 * 60)?,
            experiment_deadline: seconds(
                values,
                "GOVFOLIO_CARGO_EXPERIMENT_DEADLINE_SECONDS",
                60 * 60,
            )?,
            heartbeat: seconds(values, "GOVFOLIO_CARGO_HEARTBEAT_SECONDS", 30)?,
            progress_report: seconds(values, "GOVFOLIO_CARGO_PROGRESS_REPORT_SECONDS", 10 * 60)?,
            no_progress_deadline: seconds(values, "GOVFOLIO_CARGO_NO_PROGRESS_SECONDS", 15 * 60)?,
            focused_memory_bytes: gib(values, "GOVFOLIO_CARGO_FOCUSED_MEMORY_GIB", 4)?,
            exclusive_memory_bytes: gib(values, "GOVFOLIO_CARGO_EXCLUSIVE_MEMORY_GIB", 8)?,
            minimum_free_disk_bytes: gib(values, "GOVFOLIO_CARGO_MIN_FREE_DISK_GIB", 20)?,
            minimum_free_disk_percent: value(values, "GOVFOLIO_CARGO_MIN_FREE_DISK_PERCENT", 10)?,
        };
        let available = logical_cpus - 2;
        if config.minimum_free_disk_percent > 100 {
            return Err(BuildConfigError::InvalidValue {
                key: "GOVFOLIO_CARGO_MIN_FREE_DISK_PERCENT".to_owned(),
                value: config.minimum_free_disk_percent.to_string(),
            });
        }
        if config.focused_capacity.saturating_mul(config.focused_jobs) > available
            || config.exclusive_jobs > available
        {
            return Err(BuildConfigError::CpuBudget);
        }
        Ok(config)
    }

    #[must_use]
    pub fn jobs_for(&self, class: ResourceClass) -> usize {
        match class {
            ResourceClass::Focused => self.focused_jobs,
            ResourceClass::Exclusive => self.exclusive_jobs,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResourceSnapshot {
    pub available_memory_bytes: u64,
    pub target_free_bytes: u64,
    pub target_total_bytes: u64,
}

#[cfg(windows)]
pub fn sample_resource_snapshot(target: &Path) -> io::Result<ResourceSnapshot> {
    use std::os::windows::ffi::OsStrExt as _;

    use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
    use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};

    let mut memory = MEMORYSTATUSEX {
        dwLength: u32::try_from(std::mem::size_of::<MEMORYSTATUSEX>()).map_err(io::Error::other)?,
        ..MEMORYSTATUSEX::default()
    };
    // SAFETY: `memory` has the documented length and is valid for this call.
    if unsafe { GlobalMemoryStatusEx(&raw mut memory) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let mut wide = target.as_os_str().encode_wide().collect::<Vec<_>>();
    wide.push(0);
    let mut available = 0_u64;
    let mut total = 0_u64;
    let mut free = 0_u64;
    // SAFETY: `wide` is NUL terminated and all output pointers reference live
    // u64 values for this synchronous call.
    if unsafe {
        GetDiskFreeSpaceExW(
            wide.as_ptr(),
            &raw mut available,
            &raw mut total,
            &raw mut free,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok(ResourceSnapshot {
        available_memory_bytes: memory.ullAvailPhys,
        target_free_bytes: available,
        target_total_bytes: total,
    })
}

#[cfg(unix)]
pub fn sample_resource_snapshot(target: &Path) -> io::Result<ResourceSnapshot> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt as _;

    // SAFETY: sysconf has no pointer arguments and the queried constants return
    // scalar host values.
    let pages = unsafe { libc::sysconf(libc::_SC_AVPHYS_PAGES) };
    // SAFETY: same as above for the page-size query.
    let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
    if pages <= 0 || page_size <= 0 {
        return Err(io::Error::last_os_error());
    }
    let target = CString::new(target.as_os_str().as_bytes())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "target path contains NUL"))?;
    // SAFETY: zeroed statvfs is immediately initialized by statvfs on success.
    let mut volume = unsafe { std::mem::zeroed::<libc::statvfs>() };
    // SAFETY: `target` is NUL terminated and `volume` is a valid output pointer.
    if unsafe { libc::statvfs(target.as_ptr(), &raw mut volume) } != 0 {
        return Err(io::Error::last_os_error());
    }
    let memory = u64::try_from(pages)
        .ok()
        .and_then(|pages| {
            u64::try_from(page_size)
                .ok()
                .and_then(|size| pages.checked_mul(size))
        })
        .ok_or_else(|| io::Error::other("available memory size overflow"))?;
    Ok(ResourceSnapshot {
        available_memory_bytes: memory,
        target_free_bytes: volume.f_bavail.saturating_mul(volume.f_frsize),
        target_total_bytes: volume.f_blocks.saturating_mul(volume.f_frsize),
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueuedBuild {
    pub request_id: String,
    pub queue_sequence: i64,
    pub resource_class: ResourceClass,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RunningBuild {
    pub request_id: String,
    pub resource_class: ResourceClass,
}

#[derive(Clone, Debug)]
pub struct BuildScheduler {
    config: BuildAdmissionConfig,
}

impl BuildScheduler {
    #[must_use]
    pub fn new(config: BuildAdmissionConfig) -> Self {
        Self { config }
    }

    #[must_use]
    pub fn config(&self) -> &BuildAdmissionConfig {
        &self.config
    }

    #[must_use]
    pub fn admit(
        &self,
        queued: &[QueuedBuild],
        running: &[RunningBuild],
        resources: ResourceSnapshot,
    ) -> Vec<String> {
        if running
            .iter()
            .any(|build| build.resource_class == ResourceClass::Exclusive)
            || !self.disk_available(resources)
        {
            return Vec::new();
        }
        let mut ordered = queued.to_vec();
        ordered.sort_by_key(|build| build.queue_sequence);
        let barrier = ordered
            .iter()
            .find(|build| build.resource_class == ResourceClass::Exclusive)
            .map(|build| build.queue_sequence);
        let running_focused = running
            .iter()
            .filter(|build| build.resource_class == ResourceClass::Focused)
            .count();
        let mut admitted = Vec::new();
        let focused_slots = self.config.focused_capacity.saturating_sub(running_focused);
        for build in ordered.iter().filter(|build| {
            build.resource_class == ResourceClass::Focused
                && barrier.is_none_or(|sequence| build.queue_sequence < sequence)
        }) {
            if admitted.len() >= focused_slots {
                break;
            }
            let resulting_holders = running_focused + admitted.len() + 1;
            let required = self
                .config
                .focused_memory_bytes
                .saturating_mul(resulting_holders as u64);
            if resources.available_memory_bytes < required {
                break;
            }
            admitted.push(build.request_id.clone());
        }
        if !admitted.is_empty() || running_focused > 0 {
            return admitted;
        }
        if resources.available_memory_bytes < self.config.exclusive_memory_bytes {
            return Vec::new();
        }
        ordered
            .iter()
            .find(|build| build.resource_class == ResourceClass::Exclusive)
            .map(|build| vec![build.request_id.clone()])
            .unwrap_or_default()
    }

    fn disk_available(&self, resources: ResourceSnapshot) -> bool {
        let percent = u64::try_from(
            (u128::from(resources.target_total_bytes)
                * u128::from(self.config.minimum_free_disk_percent))
            .div_ceil(100),
        )
        .unwrap_or(u64::MAX);
        let required = self.config.minimum_free_disk_bytes.max(percent);
        resources.target_free_bytes > required
    }
}

fn value<T>(values: &BTreeMap<String, String>, key: &str, default: T) -> Result<T, BuildConfigError>
where
    T: std::str::FromStr + Copy + PartialEq + Default,
{
    match values.get(key) {
        Some(raw) => raw
            .parse::<T>()
            .ok()
            .filter(|parsed| *parsed != T::default())
            .ok_or_else(|| BuildConfigError::InvalidValue {
                key: key.to_owned(),
                value: raw.clone(),
            }),
        None => Ok(default),
    }
}

fn seconds(
    values: &BTreeMap<String, String>,
    key: &str,
    default: u64,
) -> Result<Duration, BuildConfigError> {
    value(values, key, default).map(Duration::from_secs)
}

fn gib(
    values: &BTreeMap<String, String>,
    key: &str,
    default: u64,
) -> Result<u64, BuildConfigError> {
    value(values, key, default)?
        .checked_mul(GIB)
        .ok_or_else(|| BuildConfigError::InvalidValue {
            key: key.to_owned(),
            value: "overflow".to_owned(),
        })
}

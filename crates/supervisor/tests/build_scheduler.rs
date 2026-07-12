#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;

use loop_supervisor::build_classifier::ResourceClass;
use loop_supervisor::build_scheduler::{
    BuildAdmissionConfig, BuildScheduler, QueuedBuild, ResourceSnapshot, RunningBuild,
    sample_resource_snapshot,
};

fn resources(memory_gib: u64, free_gib: u64, total_gib: u64) -> ResourceSnapshot {
    const GIB: u64 = 1024 * 1024 * 1024;
    ResourceSnapshot {
        available_memory_bytes: memory_gib * GIB,
        target_free_bytes: free_gib * GIB,
        target_total_bytes: total_gib * GIB,
    }
}

#[test]
fn build_scheduler_samples_real_host_memory_and_target_volume() {
    let temp = tempfile::tempdir().unwrap();
    let sampled = sample_resource_snapshot(temp.path()).unwrap();
    assert!(sampled.available_memory_bytes > 0);
    assert!(sampled.target_total_bytes > 0);
    assert!(sampled.target_free_bytes > 0);
    assert!(sampled.target_free_bytes <= sampled.target_total_bytes);
}

fn queued(id: &str, sequence: i64, class: ResourceClass) -> QueuedBuild {
    QueuedBuild {
        request_id: id.to_owned(),
        queue_sequence: sequence,
        resource_class: class,
    }
}

#[test]
fn build_scheduler_defaults_and_overrides_reserve_two_host_cpus() {
    let config = BuildAdmissionConfig::from_values(16, &BTreeMap::new()).unwrap();
    assert_eq!(config.focused_capacity, 2);
    assert_eq!(config.focused_jobs, 6);
    assert_eq!(config.exclusive_jobs, 14);
    assert_eq!(config.queue_deadline.as_secs(), 30 * 60);

    let invalid = BTreeMap::from([
        ("GOVFOLIO_CARGO_FOCUSED_CAPACITY".to_owned(), "3".to_owned()),
        ("GOVFOLIO_CARGO_FOCUSED_JOBS".to_owned(), "6".to_owned()),
    ]);
    assert!(BuildAdmissionConfig::from_values(16, &invalid).is_err());

    let unknown = BTreeMap::from([("GOVFOLIO_CARGO_SURPRISE".to_owned(), "1".to_owned())]);
    assert!(BuildAdmissionConfig::from_values(16, &unknown).is_err());
}

#[test]
fn build_scheduler_runs_two_focused_holders_then_honors_exclusive_barrier() {
    let scheduler =
        BuildScheduler::new(BuildAdmissionConfig::from_values(16, &BTreeMap::new()).unwrap());
    let queued = vec![
        queued("focused-1", 1, ResourceClass::Focused),
        queued("focused-2", 2, ResourceClass::Focused),
        queued("exclusive", 3, ResourceClass::Exclusive),
        queued("focused-late", 4, ResourceClass::Focused),
    ];
    assert_eq!(
        scheduler.admit(&queued, &[], resources(16, 30, 100)),
        vec!["focused-1", "focused-2"]
    );

    let running = vec![
        RunningBuild {
            request_id: "focused-1".to_owned(),
            resource_class: ResourceClass::Focused,
        },
        RunningBuild {
            request_id: "focused-2".to_owned(),
            resource_class: ResourceClass::Focused,
        },
    ];
    assert!(
        scheduler
            .admit(&queued[2..], &running, resources(16, 30, 100))
            .is_empty()
    );
    assert_eq!(
        scheduler.admit(&queued[2..], &[], resources(16, 30, 100)),
        vec!["exclusive"]
    );
}

#[test]
fn build_scheduler_allows_only_pre_barrier_focused_work_and_checks_resources() {
    let scheduler =
        BuildScheduler::new(BuildAdmissionConfig::from_values(16, &BTreeMap::new()).unwrap());
    let running = vec![RunningBuild {
        request_id: "running".to_owned(),
        resource_class: ResourceClass::Focused,
    }];
    let queued = vec![
        queued("focused-before", 2, ResourceClass::Focused),
        queued("exclusive", 3, ResourceClass::Exclusive),
        queued("focused-after", 4, ResourceClass::Focused),
    ];
    assert_eq!(
        scheduler.admit(&queued, &running, resources(16, 30, 100)),
        vec!["focused-before"]
    );
    assert!(
        scheduler
            .admit(&queued, &running, resources(7, 30, 100))
            .is_empty()
    );
    assert!(
        scheduler
            .admit(&queued, &[], resources(16, 19, 100))
            .is_empty()
    );
    assert!(
        scheduler
            .admit(&queued, &[], resources(16, 30, 400))
            .is_empty()
    );
}

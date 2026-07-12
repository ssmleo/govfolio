use std::path::Path;

use loop_supervisor::build_interference::{ObservedProcess, foreign_govfolio_processes};

fn process(pid: u32, parent_pid: u32, name: &str, command_line: &str) -> ObservedProcess {
    ObservedProcess {
        pid,
        parent_pid,
        name: name.to_owned(),
        command_line: command_line.to_owned(),
    }
}

#[test]
fn interference_filter_excludes_supervisor_tree_and_unrelated_rust_work() {
    let rows = vec![
        process(10, 0, "govfolio-loop.exe", "govfolio-loop serve-builds"),
        process(
            11,
            10,
            "cargo.exe",
            r"cargo test --target-dir C:\projects\govfolio.io\lane\target",
        ),
        process(
            12,
            11,
            "rustc.exe",
            r"rustc --out-dir C:\projects\govfolio.io\lane\target",
        ),
        process(
            20,
            0,
            "rustc.exe",
            r"rustc --out-dir C:\projects\another\target",
        ),
    ];

    assert!(
        foreign_govfolio_processes(
            &rows,
            Path::new(r"C:\projects\govfolio.io"),
            Path::new(r"C:\projects\govfolio.io\lane"),
            10,
            &[11],
        )
        .is_empty()
    );
}

#[test]
fn interference_filter_reports_foreign_govfolio_rust_without_killing_it() {
    let rows = vec![
        process(10, 0, "govfolio-loop.exe", "govfolio-loop serve-builds"),
        process(
            30,
            0,
            "cargo.exe",
            r"cargo test --manifest-path C:\projects\govfolio.io\Cargo.toml",
        ),
        process(
            31,
            30,
            "rustc.exe",
            r"rustc --out-dir C:\projects\govfolio.io\target",
        ),
    ];

    let foreign = foreign_govfolio_processes(
        &rows,
        Path::new(r"C:\projects\govfolio.io"),
        Path::new(r"C:\projects\govfolio.io\lane"),
        10,
        &[],
    );
    assert_eq!(
        foreign.iter().map(|row| row.pid).collect::<Vec<_>>(),
        [30, 31]
    );
}

#[test]
fn pathless_foreign_cargo_is_treated_as_ambiguous_host_interference() {
    let rows = vec![
        process(10, 0, "govfolio-loop.exe", "govfolio-loop serve-builds"),
        process(40, 0, "cargo.exe", "cargo test -p core"),
    ];
    let foreign = foreign_govfolio_processes(
        &rows,
        Path::new(r"C:\projects\govfolio.io"),
        Path::new(r"C:\projects\govfolio.io\lane"),
        10,
        &[],
    );
    assert_eq!(foreign.iter().map(|row| row.pid).collect::<Vec<_>>(), [40]);
}

#[test]
fn relative_manifest_path_does_not_exempt_foreign_cargo() {
    let rows = vec![process(
        41,
        0,
        "cargo.exe",
        "cargo test --manifest-path Cargo.toml",
    )];
    let foreign = foreign_govfolio_processes(
        &rows,
        Path::new(r"C:\projects\govfolio.io"),
        Path::new(r"C:\projects\govfolio.io\lane"),
        10,
        &[],
    );
    assert_eq!(foreign.iter().map(|row| row.pid).collect::<Vec<_>>(), [41]);
}

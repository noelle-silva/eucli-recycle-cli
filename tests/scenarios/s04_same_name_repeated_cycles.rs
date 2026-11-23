#[path = "../base/support/mod.rs"]
mod support;

use std::fs;
use std::time::Duration;
use support::data;
use support::run::{run_cli, run_cli_with_timeout};
use support::sandbox::Sandbox;

const BIN_SCAN_TIMEOUT: Duration = Duration::from_secs(120);

#[test]
fn repeated_same_filename_cycles_produce_distinct_ids_and_exact_restore() {
    let sb = Sandbox::new("s04_same_name_cycles");
    let dd = sb.data_dir();
    let cwd = sb.work_dir();

    // Cycle 1: create report.txt ("version 1"), recycle it
    let f = data::plain_file(&dd, "report.txt", "version 1");
    let arg = sb.checked_arg(&f);
    let p1 = run_cli(&cwd, &["put", "--format", "json", &arg]);
    assert_eq!(p1.exit_code, 0, "stdout={}", p1.stdout);
    let id_v1 = p1.json()["items"][0]["trash_id"].as_str().unwrap().to_string();

    // Cycle 2: recreate report.txt at the exact same path with different content/size, recycle it
    std::thread::sleep(Duration::from_millis(1100));
    data::plain_file(&dd, "report.txt", "version 2 with longer payload");
    let p2 = run_cli(&cwd, &["put", "--format", "json", &arg]);
    assert_eq!(p2.exit_code, 0, "stdout={}", p2.stdout);
    let id_v2 = p2.json()["items"][0]["trash_id"].as_str().unwrap().to_string();

    assert_ne!(id_v1, id_v2, "two distinct deletions of same filename must have distinct trash_ids");

    // Verify list shows BOTH versions under this sandbox folder
    let dd_s = dd.to_string_lossy().to_string();
    let list = run_cli_with_timeout(
        &cwd,
        &["list", "--from", &dd_s, "--name", "report.txt", "--format", "json"],
        BIN_SCAN_TIMEOUT,
    );
    assert_eq!(list.exit_code, 0);
    let items = list.json()["items"].as_array().unwrap().clone();
    assert_eq!(items.len(), 2, "both versions must be listed: {}", list.stdout);
    assert!(items.iter().any(|it| it["trash_id"] == id_v1));
    assert!(items.iter().any(|it| it["trash_id"] == id_v2));

    // Restore v1 specifically
    let r1 = run_cli_with_timeout(&cwd, &["restore", "--format", "json", &id_v1], BIN_SCAN_TIMEOUT);
    assert_eq!(r1.exit_code, 0, "stdout={}", r1.stdout);
    assert_eq!(fs::read_to_string(&f).unwrap(), "version 1");

    // Attempting to restore v2 now must fail with E_ORIGIN_OCCUPIED without overwriting v1
    let r2_blocked = run_cli_with_timeout(&cwd, &["restore", "--format", "json", &id_v2], BIN_SCAN_TIMEOUT);
    assert_eq!(r2_blocked.exit_code, 2, "stdout={}", r2_blocked.stdout);
    assert_eq!(r2_blocked.json()["items"][0]["code"], "E_ORIGIN_OCCUPIED");
    assert_eq!(fs::read_to_string(&f).unwrap(), "version 1");

    // Move v1 aside inside sandbox, then restore v2 cleanly
    let moved_v1 = dd.join("report_v1_saved.txt");
    fs::rename(&f, &moved_v1).unwrap();

    let r2_ok = run_cli_with_timeout(&cwd, &["restore", "--format", "json", &id_v2], BIN_SCAN_TIMEOUT);
    assert_eq!(r2_ok.exit_code, 0, "stdout={}", r2_ok.stdout);
    assert_eq!(fs::read_to_string(&f).unwrap(), "version 2 with longer payload");
    assert_eq!(fs::read_to_string(&moved_v1).unwrap(), "version 1");
}

#[test]
fn restore_two_versions_of_same_original_path_in_one_batch_is_blocked() {
    let sb = Sandbox::new("s04_same_batch_collide");
    let dd = sb.data_dir();
    let cwd = sb.work_dir();

    let f = data::plain_file(&dd, "dup_dest.txt", "first");
    let arg = sb.checked_arg(&f);
    let p1 = run_cli(&cwd, &["put", "--format", "json", &arg]);
    assert_eq!(p1.exit_code, 0);
    let id1 = p1.json()["items"][0]["trash_id"].as_str().unwrap().to_string();

    data::plain_file(&dd, "dup_dest.txt", "second_longer");
    let p2 = run_cli(&cwd, &["put", "--format", "json", &arg]);
    assert_eq!(p2.exit_code, 0);
    let id2 = p2.json()["items"][0]["trash_id"].as_str().unwrap().to_string();

    // Asking to restore both into the exact same path in a single batch must be blocked cleanly
    let r = run_cli_with_timeout(
        &cwd,
        &["restore", "--format", "json", &id1, &id2],
        BIN_SCAN_TIMEOUT,
    );
    assert_eq!(r.exit_code, 2, "stdout={}", r.stdout);
    assert_eq!(r.json()["items"][1]["code"], "E_ORIGIN_OCCUPIED");
    assert!(!f.exists(), "neither should be restored when batch has destination collision");
}

#[path = "../base/support/mod.rs"]
mod support;

use std::fs;
use std::time::Duration;
use support::data;
use support::run::{run_cli, run_cli_with_timeout};
use support::sandbox::Sandbox;

const BIN_SCAN_TIMEOUT: Duration = Duration::from_secs(120);

#[test]
fn partial_execution_failure_then_multi_restore_recovers_earlier_items() {
    let sb = Sandbox::new("s03_partial_recover");
    let dd = sb.data_dir();
    let cwd = sb.work_dir();

    let f1 = data::plain_file(&dd, "p1.txt", "payload 1");
    let f2 = data::plain_file(&dd, "p2.txt", "payload 2");
    let lock = data::locked_file(&dd, "p3_locked.txt");
    let f4 = data::plain_file(&dd, "p4_untouched.txt", "payload 4");

    let a1 = sb.checked_arg(&f1);
    let a2 = sb.checked_arg(&f2);
    let a3 = sb.checked_arg(&lock.path);
    let a4 = sb.checked_arg(&f4);

    let put = run_cli(&cwd, &["put", "--format", "json", &a1, &a2, &a3, &a4]);
    assert_eq!(put.exit_code, 3, "stdout={}", put.stdout);
    let j = put.json();
    assert_eq!(j["summary"]["succeeded"], 2);
    assert_eq!(j["summary"]["failed"], 1);
    assert_eq!(j["summary"]["not_run"], 1);

    assert!(!f1.exists());
    assert!(!f2.exists());
    assert!(lock.path.exists());
    assert!(f4.exists());

    let id1 = j["items"][0]["trash_id"].as_str().unwrap().to_string();
    let id2 = j["items"][1]["trash_id"].as_str().unwrap().to_string();

    // Release lock and batch-restore both earlier recycled items in one command
    drop(lock);
    let rest = run_cli_with_timeout(
        &cwd,
        &["restore", "--format", "json", &id1, &id2],
        BIN_SCAN_TIMEOUT,
    );
    assert_eq!(rest.exit_code, 0, "stdout={}", rest.stdout);
    let rj = rest.json();
    assert_eq!(rj["summary"]["succeeded"], 2);
    assert_eq!(fs::read_to_string(&f1).unwrap(), "payload 1");
    assert_eq!(fs::read_to_string(&f2).unwrap(), "payload 2");
}

#[test]
fn mixed_file_and_dir_batch_blocked_in_both_flag_modes() {
    let sb = Sandbox::new("s03_mixed_file_dir");
    let dd = sb.data_dir();
    let cwd = sb.work_dir();

    let f = data::plain_file(&dd, "single.txt", "keep");
    let d = data::empty_dir(&dd, "folder");
    let af = sb.checked_arg(&f);
    let ad = sb.checked_arg(&d);

    // Without --dir -> dir blocked (E_NEEDS_DIR_FLAG), file not_run
    let r1 = run_cli(&cwd, &["put", "--format", "json", &af, &ad]);
    assert_eq!(r1.exit_code, 2, "stdout={}", r1.stdout);
    let j1 = r1.json();
    assert_eq!(j1["items"][0]["outcome"], "not_run");
    assert_eq!(j1["items"][1]["outcome"], "blocked");
    assert_eq!(j1["items"][1]["code"], "E_NEEDS_DIR_FLAG");
    assert!(f.exists() && d.exists());

    // With --dir -> file blocked (E_DIR_FLAG_ON_FILE), dir not_run
    let r2 = run_cli(&cwd, &["put", "--dir", "--format", "json", &af, &ad]);
    assert_eq!(r2.exit_code, 2, "stdout={}", r2.stdout);
    let j2 = r2.json();
    assert_eq!(j2["items"][0]["outcome"], "blocked");
    assert_eq!(j2["items"][0]["code"], "E_DIR_FLAG_ON_FILE");
    assert_eq!(j2["items"][1]["outcome"], "not_run");
    assert!(f.exists() && d.exists());
}

#[test]
fn multi_restore_is_all_or_nothing_when_second_target_is_occupied() {
    let sb = Sandbox::new("s03_multi_restore_gate");
    let dd = sb.data_dir();
    let cwd = sb.work_dir();

    let f1 = data::plain_file(&dd, "m1.txt", "orig1");
    let f2 = data::plain_file(&dd, "m2.txt", "orig2");
    let a1 = sb.checked_arg(&f1);
    let a2 = sb.checked_arg(&f2);

    let put = run_cli(&cwd, &["put", "--format", "json", &a1, &a2]);
    assert_eq!(put.exit_code, 0);
    let pj = put.json();
    let id1 = pj["items"][0]["trash_id"].as_str().unwrap().to_string();
    let id2 = pj["items"][1]["trash_id"].as_str().unwrap().to_string();

    // Occupy f2 only
    fs::write(&f2, b"new occupant").unwrap();

    let rest = run_cli_with_timeout(
        &cwd,
        &["restore", "--format", "json", &id1, &id2],
        BIN_SCAN_TIMEOUT,
    );
    assert_eq!(rest.exit_code, 2, "stdout={}", rest.stdout);
    let rj = rest.json();
    assert_eq!(rj["items"][0]["outcome"], "not_run");
    assert_eq!(rj["items"][1]["outcome"], "blocked");
    assert_eq!(rj["items"][1]["code"], "E_ORIGIN_OCCUPIED");
    assert!(!f1.exists(), "all-or-nothing: first entry must NOT have been restored");
    assert_eq!(fs::read_to_string(&f2).unwrap(), "new occupant");
}

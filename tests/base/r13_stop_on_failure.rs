mod support;

use support::data;
use support::run::run_cli;
use support::sandbox::Sandbox;

#[test]
fn stops_immediately_on_execution_failure_without_rollback() {
    let sb = Sandbox::new("r13_stop_on_fail");
    let dd = sb.data_dir();
    let cwd = sb.work_dir();

    let f1 = data::plain_file(&dd, "first_ok.txt", "will succeed");
    let lock_guard = data::locked_file(&dd, "second_locked.txt");
    let f3 = data::plain_file(&dd, "third_unrun.txt", "must not be touched");

    let a1 = sb.checked_arg(&f1);
    let a2 = sb.checked_arg(&lock_guard.path);
    let a3 = sb.checked_arg(&f3);

    let res = run_cli(&cwd, &["put", "--format", "json", &a1, &a2, &a3]);
    assert_eq!(res.exit_code, 3, "stdout={}", res.stdout);

    let j = res.json();
    assert_eq!(j["summary"]["total"], 3);
    assert_eq!(j["summary"]["succeeded"], 1);
    assert_eq!(j["summary"]["failed"], 1);
    assert_eq!(j["summary"]["not_run"], 1);

    assert_eq!(j["items"][0]["outcome"], "succeeded");
    assert_eq!(j["items"][0]["code"], "E_OK");
    assert!(!f1.exists(), "first file should remain in recycle bin (no rollback)");

    assert_eq!(j["items"][1]["outcome"], "failed");
    assert_eq!(j["items"][1]["code"], "E_IN_USE");
    assert!(lock_guard.path.exists(), "locked file must remain in place");

    assert_eq!(j["items"][2]["outcome"], "not_run");
    assert_eq!(j["items"][2]["code"], "E_STOPPED");
    assert!(f3.exists(), "R13 violation: third file must not have been executed");

    drop(lock_guard);
}

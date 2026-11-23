mod support;

use support::data;
use support::run::run_cli;
use support::sandbox::Sandbox;

#[test]
fn single_invalid_target_blocks_entire_batch() {
    let sb = Sandbox::new("r12_batch");
    let dd = sb.data_dir();
    let cwd = sb.work_dir();

    let f1 = data::plain_file(&dd, "first.txt", "1");
    let f3 = data::plain_file(&dd, "third.txt", "3");
    let a1 = sb.checked_arg(&f1);
    let a3 = sb.checked_arg(&f3);

    let res = run_cli(&cwd, &["put", "--format", "json", &a1, "relative_bad.txt", &a3]);
    assert_eq!(res.exit_code, 2, "stdout={}", res.stdout);
    let j = res.json();
    assert_eq!(j["items"].as_array().unwrap().len(), 3);
    assert_eq!(j["items"][0]["outcome"], "not_run");
    assert_eq!(j["items"][1]["outcome"], "blocked");
    assert_eq!(j["items"][1]["code"], "E_NOT_ABSOLUTE");
    assert_eq!(j["items"][2]["outcome"], "not_run");

    assert!(f1.exists(), "R12 violation: first target must not be touched");
    assert!(f3.exists(), "R12 violation: third target must not be touched");
}

#[test]
fn dry_run_plans_all_valid_targets_without_touching_disk() {
    let sb = Sandbox::new("r12_dry_run_ok");
    let dd = sb.data_dir();
    let cwd = sb.work_dir();

    let f1 = data::plain_file(&dd, "plan1.txt", "1");
    let f2 = data::plain_file(&dd, "plan2.txt", "2");
    let a1 = sb.checked_arg(&f1);
    let a2 = sb.checked_arg(&f2);

    let res = run_cli(&cwd, &["put", "--dry-run", "--format", "json", &a1, &a2]);
    assert_eq!(res.exit_code, 0, "stdout={}", res.stdout);
    let j = res.json();
    assert_eq!(j["items"][0]["outcome"], "planned");
    assert_eq!(j["items"][1]["outcome"], "planned");
    assert!(f1.exists(), "dry-run must not remove files");
    assert!(f2.exists(), "dry-run must not remove files");
}

#[test]
fn dry_run_reports_blocked_with_exit_2() {
    let sb = Sandbox::new("r12_dry_run_blocked");
    let dd = sb.data_dir();
    let cwd = sb.work_dir();

    let f1 = data::plain_file(&dd, "plan1.txt", "1");
    let dir2 = data::empty_dir(&dd, "needs_flag_dir");
    let a1 = sb.checked_arg(&f1);
    let a2 = sb.checked_arg(&dir2);

    let res = run_cli(&cwd, &["put", "--dry-run", "--format", "json", &a1, &a2]);
    assert_eq!(res.exit_code, 2, "stdout={}", res.stdout);
    let j = res.json();
    assert_eq!(j["items"][1]["outcome"], "blocked");
    assert_eq!(j["items"][1]["code"], "E_NEEDS_DIR_FLAG");
    assert!(f1.exists());
    assert!(dir2.exists());
}

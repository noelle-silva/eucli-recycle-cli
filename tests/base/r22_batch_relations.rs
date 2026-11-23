mod support;

use support::data;
use support::run::run_cli;
use support::sandbox::Sandbox;

#[test]
fn rejects_duplicate_targets_with_case_and_separator_variants() {
    let sb = Sandbox::new("r22_dup");
    let dd = sb.data_dir();
    let cwd = sb.work_dir();

    let f = data::plain_file(&dd, "DupTarget.txt", "keep");
    let a1 = sb.checked_arg(&f);
    let a2 = a1.to_lowercase().replace('\\', "/");

    let res = run_cli(&cwd, &["put", "--format", "json", &a1, &a2]);
    assert_eq!(res.exit_code, 2, "stdout={}", res.stdout);
    let j = res.json();
    let has_dup = j["items"]
        .as_array()
        .unwrap()
        .iter()
        .any(|it| it["code"] == "E_DUPLICATE_TARGET");
    assert!(has_dup, "expected E_DUPLICATE_TARGET in {}", res.stdout);
    assert!(f.exists(), "file must remain untouched");
}

#[test]
fn rejects_overlapping_parent_and_child_targets_in_either_order() {
    let sb = Sandbox::new("r22_overlap");
    let dd = sb.data_dir();
    let cwd = sb.work_dir();

    let dir = data::nested_dir(&dd, "parent_dir");
    let child = dir.join("top.txt");
    let a_dir = sb.checked_arg(&dir);
    let a_child = sb.checked_arg(&child);

    // Parent then child
    let r1 = run_cli(&cwd, &["put", "--dir", "--format", "json", &a_dir, &a_child]);
    assert_eq!(r1.exit_code, 2, "stdout={}", r1.stdout);
    assert!(
        r1.json()["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|it| it["code"] == "E_OVERLAPPING_TARGET"),
        "expected E_OVERLAPPING_TARGET: {}",
        r1.stdout
    );
    assert!(dir.exists() && child.exists());

    // Child then parent
    let r2 = run_cli(&cwd, &["put", "--dir", "--format", "json", &a_child, &a_dir]);
    assert_eq!(r2.exit_code, 2, "stdout={}", r2.stdout);
    assert!(
        r2.json()["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|it| it["code"] == "E_OVERLAPPING_TARGET"),
        "expected E_OVERLAPPING_TARGET: {}",
        r2.stdout
    );
    assert!(dir.exists() && child.exists());
}

#[test]
fn rejects_more_than_1000_targets() {
    let sb = Sandbox::new("r22_too_many");
    let cwd = sb.work_dir();

    // Use short non-executable synthetic paths so the total command line stays under Windows'
    // 32,767-char CreateProcessW limit; step 0 rejects >1000 targets before touching anything.
    let owned: Vec<String> = (0..1001).map(|i| format!(r"E:\x\{}", i)).collect();
    let mut args: Vec<&str> = vec!["put", "--format", "json"];
    for s in &owned {
        args.push(s.as_str());
    }

    let res = run_cli(&cwd, &args);
    assert_eq!(res.exit_code, 1, "stdout={}", res.stdout);
    assert_eq!(res.json()["error"]["code"], "E_TOO_MANY_TARGETS");
}

#[test]
fn rejects_duplicate_trash_ids_in_restore() {
    let sb = Sandbox::new("r22_dup_restore");
    let cwd = sb.work_dir();

    let res = run_cli(
        &cwd,
        &["restore", "--format", "json", "abcdef0123456789", "ABCDEF0123456789"],
    );
    assert_eq!(res.exit_code, 2, "stdout={}", res.stdout);
    let j = res.json();
    assert!(
        j["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|it| it["code"] == "E_DUPLICATE_TRASH_ID"),
        "expected E_DUPLICATE_TRASH_ID: {}",
        res.stdout
    );
}

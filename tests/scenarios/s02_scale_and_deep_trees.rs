#[path = "../base/support/mod.rs"]
mod support;

use std::fs;
use std::time::Duration;
use support::data;
use support::run::{run_cli, run_cli_with_timeout};
use support::sandbox::Sandbox;

const BIN_SCAN_TIMEOUT: Duration = Duration::from_secs(120);

#[test]
fn directory_with_300_small_files_put_and_restore_lossless() {
    let sb = Sandbox::new("s02_300_files");
    let dd = sb.data_dir();
    let cwd = sb.work_dir();

    let dir = data::many_small_files(&dd, "bulk_300", 300);
    let arg = sb.checked_arg(&dir);

    let put = run_cli(&cwd, &["put", "--dir", "--format", "json", &arg]);
    assert_eq!(put.exit_code, 0, "stdout={} stderr={}", put.stdout, put.stderr);
    assert!(!dir.exists(), "300-file dir should be recycled");
    let tid = put.json()["items"][0]["trash_id"].as_str().unwrap().to_string();

    let rest = run_cli_with_timeout(&cwd, &["restore", "--format", "json", &tid], BIN_SCAN_TIMEOUT);
    assert_eq!(rest.exit_code, 0, "stdout={}", rest.stdout);
    assert!(dir.exists());

    for i in 0..300 {
        let f = dir.join(format!("f_{:04}.txt", i));
        assert!(f.exists(), "missing restored file #{}", i);
    }
    assert_eq!(
        fs::read_to_string(dir.join("f_0299.txt")).unwrap(),
        "small file 299"
    );
}

#[test]
fn large_file_20mib_put_and_restore_preserves_size() {
    let sb = Sandbox::new("s02_large_file");
    let dd = sb.data_dir();
    let cwd = sb.work_dir();

    let size_bytes: u64 = 20 * 1024 * 1024; // 20 MiB
    let big = data::large_file(&dd, "blob_20m.bin", size_bytes);
    let arg = sb.checked_arg(&big);

    let put = run_cli(&cwd, &["put", "--format", "json", &arg]);
    assert_eq!(put.exit_code, 0, "stdout={} stderr={}", put.stdout, put.stderr);
    assert!(!big.exists());
    let tid = put.json()["items"][0]["trash_id"].as_str().unwrap().to_string();

    let rest = run_cli_with_timeout(&cwd, &["restore", "--format", "json", &tid], BIN_SCAN_TIMEOUT);
    assert_eq!(rest.exit_code, 0, "stdout={}", rest.stdout);
    let meta = fs::metadata(&big).expect("restored large file metadata");
    assert_eq!(meta.len(), size_bytes);
}

#[test]
fn moderate_deep_tree_put_and_restore() {
    let sb = Sandbox::new("s02_safe_deep");
    let dd = sb.data_dir();
    let cwd = sb.work_dir();

    // 12 nested short segments stay safely under 260 chars and should roundtrip cleanly
    let tree = data::deep_tree(&dd, "deep12", 12);
    let arg = sb.checked_arg(&tree);

    let put = run_cli(&cwd, &["put", "--dir", "--format", "json", &arg]);
    assert_eq!(put.exit_code, 0, "stdout={}", put.stdout);
    assert!(!tree.exists());
    let tid = put.json()["items"][0]["trash_id"].as_str().unwrap().to_string();

    let rest = run_cli_with_timeout(&cwd, &["restore", "--format", "json", &tid], BIN_SCAN_TIMEOUT);
    assert_eq!(rest.exit_code, 0, "stdout={}", rest.stdout);

    let mut cur = tree.clone();
    for i in 0..12 {
        cur.push(format!("d{}", i));
    }
    assert_eq!(fs::read_to_string(cur.join("end.txt")).unwrap(), "deep");
}

#[test]
fn mixed_dir_with_normal_sibling_and_overlong_branch_preserves_everything() {
    let sb = Sandbox::new("s02_mixed_long");
    let dd = sb.data_dir();
    let cwd = sb.work_dir();

    let (top, leaf_ext) = data::long_path(&dd, "mixed_long_root");
    let normal_sibling = data::plain_file(&top, "normal_sibling.txt", "do not lose me");
    let arg = sb.checked_arg(&top);

    let put = run_cli(&cwd, &["put", "--dir", "--format", "json", &arg]);
    assert_eq!(put.exit_code, 3, "stdout={}", put.stdout);
    assert_eq!(put.json()["items"][0]["outcome"], "failed");
    assert_eq!(put.json()["items"][0]["code"], "E_RECYCLE_REFUSED");
    assert!(
        normal_sibling.exists(),
        "normal sibling inside refused long-path dir must NOT be partially deleted!"
    );
    assert!(
        leaf_ext.exists(),
        "deep leaf inside refused long-path dir must remain intact!"
    );
}

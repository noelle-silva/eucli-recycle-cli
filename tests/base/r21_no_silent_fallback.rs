mod support;

use std::fs;
use support::data;
use support::probe::all_src_rs_files;
use support::run::run_cli;
use support::sandbox::Sandbox;

#[test]
fn static_no_retry_or_fallback_patterns_in_platform_operations() {
    for file in all_src_rs_files() {
        if file.to_string_lossy().contains("platform") {
            let content = fs::read_to_string(&file).expect("read platform file");
            // Ensure .PerformOperations() is invoked at most once in recycle and once in restore, never inside retry loops
            let count = content.matches(".PerformOperations(").count();
            assert!(
                count <= 2,
                "R21 check: unexpected multiple PerformOperations calls ({}) in {}",
                count,
                file.display()
            );
            assert!(
                !content.contains("std::thread::sleep"),
                "R21 check: retry sleep detected in {}",
                file.display()
            );
        }
    }
}

#[test]
fn failures_fail_honestly_without_silent_fallback() {
    let sb = Sandbox::new("r21_no_fallback");
    let dd = sb.data_dir();
    let cwd = sb.work_dir();

    // 1. Long path fails instead of falling back to permanent delete
    let (long_dir, leaf) = data::long_path(&dd, "r21_long");
    let r1 = run_cli(&cwd, &["put", "--dir", "--format", "json", &sb.checked_arg(&long_dir)]);
    assert_eq!(r1.exit_code, 3);
    assert_eq!(r1.json()["items"][0]["outcome"], "failed");
    assert!(leaf.exists());

    // 2. Corrupted config fails instead of silently falling back to defaults
    let bad_cfg = sb.path().join("corrupt.toml");
    fs::write(&bad_cfg, "not valid toml = = =").unwrap();
    let f = data::plain_file(&dd, "stay.txt", "stay");
    let r2 = run_cli(
        &cwd,
        &[
            "put",
            "--config",
            &bad_cfg.to_string_lossy(),
            "--format",
            "json",
            &sb.checked_arg(&f),
        ],
    );
    assert_eq!(r2.exit_code, 1);
    assert_eq!(r2.json()["error"]["code"], "E_CONFIG_PARSE");
    assert!(f.exists());
}

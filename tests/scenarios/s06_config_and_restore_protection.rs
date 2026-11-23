#[path = "../base/support/mod.rs"]
mod support;

use std::fs;
use std::time::Duration;
use support::data;
use support::run::{run_cli, run_cli_with_timeout};
use support::sandbox::Sandbox;

const BIN_SCAN_TIMEOUT: Duration = Duration::from_secs(120);

#[test]
fn relative_config_path_resolves_against_cwd_and_blocks_protected_restore() {
    let sb = Sandbox::new("s06_config_restore");
    let dd = sb.data_dir();
    let cwd = sb.work_dir();

    let vault = data::empty_dir(&dd, "vault");
    let doc = data::plain_file(&vault, "secret.txt", "top secret");
    let doc_arg = sb.checked_arg(&doc);

    // Put relative config file inside cwd
    let cfg_path = cwd.join("local_rules.toml");
    fs::write(&cfg_path, "schema = 1\n").unwrap();

    // 1. Recycle before protection is added (using relative --config)
    let put = run_cli(
        &cwd,
        &["put", "--config", r".\local_rules.toml", "--format", "json", &doc_arg],
    );
    assert_eq!(put.exit_code, 0, "stdout={}", put.stdout);
    let tid = put.json()["items"][0]["trash_id"].as_str().unwrap().to_string();
    assert!(!doc.exists());

    // 2. Add `vault` to protect.tree -> restore must be blocked (Section 8)
    let protected_toml = format!(
        "schema = 1\n\n[protect.tree]\npaths = [\"{}\"]\n",
        vault.to_string_lossy().replace('\\', r"\\")
    );
    fs::write(&cfg_path, protected_toml).unwrap();

    let rest_blocked = run_cli_with_timeout(
        &cwd,
        &["restore", "--config", r".\local_rules.toml", "--format", "json", &tid],
        BIN_SCAN_TIMEOUT,
    );
    assert_eq!(rest_blocked.exit_code, 2, "stdout={}", rest_blocked.stdout);
    assert_eq!(rest_blocked.json()["items"][0]["outcome"], "blocked");
    assert_eq!(rest_blocked.json()["items"][0]["code"], "E_PROTECTED_PARENT");
    assert!(!doc.exists(), "protected restore must not write file back");

    // 3. Remove protection -> restore succeeds immediately on next call (R19)
    fs::write(&cfg_path, "schema = 1\n").unwrap();
    let rest_ok = run_cli_with_timeout(
        &cwd,
        &["restore", "--config", r".\local_rules.toml", "--format", "json", &tid],
        BIN_SCAN_TIMEOUT,
    );
    assert_eq!(rest_ok.exit_code, 0, "stdout={}", rest_ok.stdout);
    assert_eq!(fs::read_to_string(&doc).unwrap(), "top secret");
}

#[test]
fn protect_self_allows_child_put_and_restore_while_blocking_dir_itself() {
    let sb = Sandbox::new("s06_self_roundtrip");
    let dd = sb.data_dir();
    let cwd = sb.work_dir();

    let self_dir = data::empty_dir(&dd, "home_like");
    let child = data::plain_file(&self_dir, "temp_note.txt", "note body");
    let cfg = sb.path().join("self_only.toml");
    let toml_body = format!(
        "schema = 1\n\n[protect.self]\npaths = [\"{}\"]\n",
        self_dir.to_string_lossy().replace('\\', r"\\")
    );
    fs::write(&cfg, toml_body).unwrap();
    let cfg_s = cfg.to_string_lossy().to_string();

    // Dir itself blocked
    let r_dir = run_cli(
        &cwd,
        &["put", "--dir", "--config", &cfg_s, "--format", "json", &sb.checked_arg(&self_dir)],
    );
    assert_eq!(r_dir.exit_code, 2);
    assert_eq!(r_dir.json()["items"][0]["code"], "E_BLACKLIST_SELF");

    // Child put allowed
    let r_put = run_cli(
        &cwd,
        &["put", "--config", &cfg_s, "--format", "json", &sb.checked_arg(&child)],
    );
    assert_eq!(r_put.exit_code, 0, "stdout={}", r_put.stdout);
    let tid = r_put.json()["items"][0]["trash_id"].as_str().unwrap().to_string();

    // Child restore back into protect.self dir also allowed
    let r_rest = run_cli_with_timeout(
        &cwd,
        &["restore", "--config", &cfg_s, "--format", "json", &tid],
        BIN_SCAN_TIMEOUT,
    );
    assert_eq!(r_rest.exit_code, 0, "stdout={}", r_rest.stdout);
    assert_eq!(fs::read_to_string(&child).unwrap(), "note body");
}

mod support;

use std::fs;
use support::data;
use support::run::run_cli;
use support::sandbox::Sandbox;

#[test]
fn live_reloads_config_between_invocations() {
    let sb = Sandbox::new("r19_live_reload");
    let dd = sb.data_dir();
    let cwd = sb.work_dir();

    let zone = data::empty_dir(&dd, "dynamic_zone");
    let f1 = data::plain_file(&zone, "a.txt", "a");
    let f2 = data::plain_file(&zone, "b.txt", "b");
    let cfg = sb.path().join("live.toml");
    let cfg_s = cfg.to_string_lossy().to_string();

    // First config: empty protections -> f1 allowed
    fs::write(&cfg, "schema = 1\n").unwrap();
    let r1 = run_cli(
        &cwd,
        &["put", "--config", &cfg_s, "--format", "json", &sb.checked_arg(&f1)],
    );
    assert_eq!(r1.exit_code, 0, "stdout={}", r1.stdout);
    assert!(!f1.exists());

    // Immediately add zone to protect.tree -> f2 must be blocked on the very next call
    let updated = format!(
        "schema = 1\n\n[protect.tree]\npaths = [\"{}\"]\n",
        zone.to_string_lossy().replace('\\', r"\\")
    );
    fs::write(&cfg, updated).unwrap();
    let r2 = run_cli(
        &cwd,
        &["put", "--config", &cfg_s, "--format", "json", &sb.checked_arg(&f2)],
    );
    assert_eq!(r2.exit_code, 2, "stdout={}", r2.stdout);
    assert_eq!(r2.json()["items"][0]["code"], "E_PROTECTED_PARENT");
    assert!(f2.exists());
}

#[test]
fn rejects_invalid_or_missing_config_without_fallback() {
    let sb = Sandbox::new("r19_invalid_cfg");
    let dd = sb.data_dir();
    let cwd = sb.work_dir();
    let f = data::plain_file(&dd, "safe.txt", "safe");
    let arg = sb.checked_arg(&f);

    // 1. Explicit --config missing -> E_CONFIG_NOT_FOUND
    let missing_cfg = sb.path().join("no_such.toml").to_string_lossy().to_string();
    let r_miss = run_cli(&cwd, &["put", "--config", &missing_cfg, "--format", "json", &arg]);
    assert_eq!(r_miss.exit_code, 1);
    assert_eq!(r_miss.json()["error"]["code"], "E_CONFIG_NOT_FOUND");

    // 2. Syntax error -> E_CONFIG_PARSE
    let bad_toml = sb.path().join("bad_syntax.toml");
    fs::write(&bad_toml, "schema = [[[broken").unwrap();
    let r_parse = run_cli(
        &cwd,
        &["put", "--config", &bad_toml.to_string_lossy(), "--format", "json", &arg],
    );
    assert_eq!(r_parse.exit_code, 1);
    assert_eq!(r_parse.json()["error"]["code"], "E_CONFIG_PARSE");

    // 3. Relative path in config -> E_CONFIG_VALUE
    let bad_rel = sb.path().join("bad_rel.toml");
    fs::write(&bad_rel, "schema = 1\n[protect.tree]\npaths = [\"relative\\\\dir\"]\n").unwrap();
    let r_rel = run_cli(
        &cwd,
        &["put", "--config", &bad_rel.to_string_lossy(), "--format", "json", &arg],
    );
    assert_eq!(r_rel.exit_code, 1);
    assert_eq!(r_rel.json()["error"]["code"], "E_CONFIG_VALUE");

    // 4. Duplicate path (case-insensitive) -> E_CONFIG_VALUE
    let bad_dup = sb.path().join("bad_dup.toml");
    fs::write(
        &bad_dup,
        "schema = 1\n[protect.tree]\npaths = [\"E:\\\\work\\\\a\", \"e:\\\\WORK\\\\A\"]\n",
    )
    .unwrap();
    let r_dup = run_cli(
        &cwd,
        &["put", "--config", &bad_dup.to_string_lossy(), "--format", "json", &arg],
    );
    assert_eq!(r_dup.exit_code, 1);
    assert_eq!(r_dup.json()["error"]["code"], "E_CONFIG_VALUE");

    // 5. Overlap between tree and self -> E_CONFIG_VALUE
    let bad_ov = sb.path().join("bad_overlap.toml");
    fs::write(
        &bad_ov,
        "schema = 1\n[protect.tree]\npaths = [\"E:\\\\work\"]\n[protect.self]\npaths = [\"E:\\\\work\\\\sub\"]\n",
    )
    .unwrap();
    let r_ov = run_cli(
        &cwd,
        &["put", "--config", &bad_ov.to_string_lossy(), "--format", "json", &arg],
    );
    assert_eq!(r_ov.exit_code, 1);
    assert_eq!(r_ov.json()["error"]["code"], "E_CONFIG_VALUE");

    // 6. Drive root in config -> E_CONFIG_VALUE
    let bad_root = sb.path().join("bad_root.toml");
    fs::write(&bad_root, "schema = 1\n[protect.self]\npaths = [\"E:\\\\\"]\n").unwrap();
    let r_root = run_cli(
        &cwd,
        &["put", "--config", &bad_root.to_string_lossy(), "--format", "json", &arg],
    );
    assert_eq!(r_root.exit_code, 1);
    assert_eq!(r_root.json()["error"]["code"], "E_CONFIG_VALUE");

    // 7. Unsupported schema version -> E_CONFIG_VALUE
    let bad_schema = sb.path().join("bad_schema.toml");
    fs::write(&bad_schema, "schema = 99\n").unwrap();
    let r_schema = run_cli(
        &cwd,
        &["put", "--config", &bad_schema.to_string_lossy(), "--format", "json", &arg],
    );
    assert_eq!(r_schema.exit_code, 1);
    assert_eq!(r_schema.json()["error"]["code"], "E_CONFIG_VALUE");

    assert!(f.exists(), "file must remain untouched across all config failures");
}

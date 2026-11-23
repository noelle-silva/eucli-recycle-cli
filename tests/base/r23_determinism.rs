mod support;

use support::data;
use support::run::{run_cli, run_cli_with_options, DEFAULT_TIMEOUT};
use support::sandbox::Sandbox;

#[test]
fn output_is_deterministic_across_cwd_and_environment_variables() {
    let sb = Sandbox::new("r23_determinism");
    let dd = sb.data_dir();
    let cwd_a = data::empty_dir(sb.path(), "cwd_a");
    let cwd_b = data::empty_dir(sb.path(), "cwd_b");

    let target = data::plain_file(&dd, "det.txt", "deterministic");
    let arg = sb.checked_arg(&target);

    // Run 1: cwd_a, cleared env
    let r1 = run_cli_with_options(
        &cwd_a,
        &["put", "--dry-run", "--format", "json", &arg],
        DEFAULT_TIMEOUT,
        &[],
        true,
    );
    // Run 2: cwd_b, noisy locale/env variables
    let r2 = run_cli_with_options(
        &cwd_b,
        &["put", "--dry-run", "--format", "json", &arg],
        DEFAULT_TIMEOUT,
        &[
            ("LANG", "ja_JP.UTF-8"),
            ("LC_ALL", "C"),
            ("EUCLI_RECYCLE_FAKE", "1"),
        ],
        false,
    );

    assert_eq!(r1.exit_code, 0);
    assert_eq!(r1.exit_code, r2.exit_code);
    let j1 = r1.json();
    let j2 = r2.json();
    assert_eq!(j1["command"], j2["command"]);
    assert_eq!(j1["exit_code"], j2["exit_code"]);
    assert_eq!(j1["summary"], j2["summary"]);
    assert_eq!(j1["items"], j2["items"]);

    // --help and --version are byte-for-byte identical regardless of env
    let h1 = run_cli_with_options(&cwd_a, &["--help"], DEFAULT_TIMEOUT, &[], true);
    let h2 = run_cli_with_options(&cwd_b, &["--help"], DEFAULT_TIMEOUT, &[("LANG", "en_US")], false);
    assert_eq!(h1.stdout, h2.stdout);
    assert_eq!(h1.exit_code, h2.exit_code);

    let v1 = run_cli_with_options(&cwd_a, &["--version"], DEFAULT_TIMEOUT, &[], true);
    let v2 = run_cli_with_options(&cwd_b, &["--version"], DEFAULT_TIMEOUT, &[], false);
    assert_eq!(v1.stdout, v2.stdout);
}

#[test]
fn handles_weird_utf8_and_special_filenames_cleanly() {
    let sb = Sandbox::new("r23_weird_names");
    let dd = sb.data_dir();
    let cwd = sb.work_dir();
    let files = data::weird_names(&dd);

    let owned: Vec<String> = files.iter().map(|p| sb.checked_arg(p)).collect();
    let mut args = vec!["put", "--format", "json"];
    for s in &owned {
        args.push(s.as_str());
    }

    let res = run_cli(&cwd, &args);
    assert_eq!(res.exit_code, 0, "stdout={} stderr={}", res.stdout, res.stderr);
    let j = res.json();
    assert_eq!(j["summary"]["succeeded"], files.len() as u64);
    for f in &files {
        assert!(!f.exists(), "weird file {:?} should be recycled", f);
    }
}

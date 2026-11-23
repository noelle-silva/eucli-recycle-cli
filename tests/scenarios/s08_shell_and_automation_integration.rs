#[path = "../base/support/mod.rs"]
mod support;

use std::os::windows::process::CommandExt;
use std::process::Command;
use std::time::Duration;
use support::data;
use support::run::{bin_path, run_cli, run_cli_with_piped_stdin};
use support::sandbox::Sandbox;

#[test]
fn piped_stdin_is_ignored_and_never_hangs_in_automation() {
    let sb = Sandbox::new("s08_piped_stdin");
    let dd = sb.data_dir();
    let cwd = sb.work_dir();

    let noise = vec![b'y'; 16 * 1024];

    // 1. Normal put with open piped stdin
    let f = data::plain_file(&dd, "pipe_ok.txt", "ok");
    let r1 = run_cli_with_piped_stdin(
        &cwd,
        &["put", "--format", "json", &sb.checked_arg(&f)],
        &noise,
        Duration::from_secs(30),
    );
    assert_eq!(r1.exit_code, 0, "stdout={}", r1.stdout);
    assert!(r1.stderr.is_empty(), "stderr must be empty in json mode");
    assert!(!f.exists());

    // 2. Failing long-path put with "yyyy..." piped to stdin (must still refuse, never accept!)
    let (long_dir, leaf) = data::long_path(&dd, "pipe_long");
    let r2 = run_cli_with_piped_stdin(
        &cwd,
        &["put", "--dir", "--format", "json", &sb.checked_arg(&long_dir)],
        b"y\ny\ny\n",
        Duration::from_secs(30),
    );
    assert_eq!(r2.exit_code, 3, "stdout={}", r2.stdout);
    assert_eq!(r2.json()["items"][0]["code"], "E_RECYCLE_REFUSED");
    assert!(r2.stderr.is_empty(), "stderr must be empty in json mode");
    assert!(leaf.exists(), "piped 'y' must NEVER trigger permanent deletion");
}

#[test]
fn json_mode_keeps_stderr_completely_empty_across_all_exit_codes() {
    let sb = Sandbox::new("s08_stderr_clean");
    let dd = sb.data_dir();
    let cwd = sb.work_dir();

    // Exit 0 (dry-run planned)
    let f = data::plain_file(&dd, "ok.txt", "ok");
    let r0 = run_cli(&cwd, &["put", "--dry-run", "--format", "json", &sb.checked_arg(&f)]);
    assert_eq!(r0.exit_code, 0);
    assert!(r0.stderr.is_empty(), "exit 0 stderr={:?}", r0.stderr);
    let _ = r0.json();

    // Exit 1 (parse error)
    let r1 = run_cli(&cwd, &["put", "-rf", "--format", "json", &sb.checked_arg(&f)]);
    assert_eq!(r1.exit_code, 1);
    assert!(r1.stderr.is_empty(), "exit 1 stderr={:?}", r1.stderr);
    let _ = r1.json();

    // Exit 2 (blocked)
    let r2 = run_cli(&cwd, &["put", "--dir", "--format", "json", &sb.checked_arg(&f)]);
    assert_eq!(r2.exit_code, 2);
    assert!(r2.stderr.is_empty(), "exit 2 stderr={:?}", r2.stderr);
    let _ = r2.json();

    // Exit 3 (failed — locked file)
    let lock = data::locked_file(&dd, "locked_err.txt");
    let r3 = run_cli(&cwd, &["put", "--format", "json", &sb.checked_arg(&lock.path)]);
    assert_eq!(r3.exit_code, 3);
    assert!(r3.stderr.is_empty(), "exit 3 stderr={:?}", r3.stderr);
    let _ = r3.json();
    drop(lock);
}

#[test]
fn invoked_from_pwsh_and_cmd_with_spaces_and_unicode_paths() {
    let sb = Sandbox::new("s08_shells");
    let dd = sb.data_dir();
    let cwd = sb.work_dir();

    // 1. Invoked via pwsh with spaces + Chinese filename
    let f_ps = data::plain_file(&dd, "pwsh 空间 中文.txt", "from pwsh");
    let arg_ps = sb.checked_arg(&f_ps);
    let out_ps = Command::new("pwsh")
        .current_dir(&cwd)
        .args([
            "-NoLogo",
            "-NoProfile",
            "-Command",
            &format!("& '{}' put --format json '{}'", bin_path(), arg_ps),
        ])
        .output()
        .expect("invoke via pwsh");
    assert_eq!(
        out_ps.status.code(),
        Some(0),
        "pwsh stdout={} stderr={}",
        String::from_utf8_lossy(&out_ps.stdout),
        String::from_utf8_lossy(&out_ps.stderr)
    );
    let j_ps: serde_json::Value =
        serde_json::from_slice(&out_ps.stdout).expect("valid utf-8 json from pwsh");
    assert_eq!(j_ps["items"][0]["outcome"], "succeeded");
    assert!(!f_ps.exists());

    // 2. Invoked via cmd.exe with spaces + parentheses (use raw_arg so Rust doesn't backslash-escape quotes for cmd.exe)
    let f_cmd = data::plain_file(&dd, "cmd file (1).txt", "from cmd");
    let arg_cmd = sb.checked_arg(&f_cmd);
    let out_cmd = Command::new("cmd.exe")
        .current_dir(&cwd)
        .arg("/C")
        .raw_arg(format!("\"\"{}\" put --format json \"{}\"\"", bin_path(), arg_cmd))
        .output()
        .expect("invoke via cmd.exe");
    assert_eq!(
        out_cmd.status.code(),
        Some(0),
        "cmd stdout={} stderr={}",
        String::from_utf8_lossy(&out_cmd.stdout),
        String::from_utf8_lossy(&out_cmd.stderr)
    );
    let j_cmd: serde_json::Value =
        serde_json::from_slice(&out_cmd.stdout).expect("valid utf-8 json from cmd.exe");
    assert_eq!(j_cmd["items"][0]["outcome"], "succeeded");
    assert!(!f_cmd.exists());
}

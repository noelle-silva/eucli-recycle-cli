mod support;

use support::data;
use support::probe::all_src_rs_files;
use support::run::run_cli;
use support::sandbox::Sandbox;

#[test]
fn static_no_permanent_delete_calls_in_source() {
    let forbidden = [
        "remove_file",
        "remove_dir",
        "remove_dir_all",
        "DeleteFileW",
        "DeleteFileA",
        "RemoveDirectoryW",
        "RemoveDirectoryA",
        "SHFileOperation",
        "FOF_WANTNUKEWARNING",
    ];
    for file in all_src_rs_files() {
        let content = std::fs::read_to_string(&file).expect("read src file");
        for pat in forbidden {
            assert!(
                !content.contains(pat),
                "R1 violation: forbidden permanent-delete symbol `{}` found in {}",
                pat,
                file.display()
            );
        }
    }
}

#[test]
fn rejects_unknown_subcommands() {
    let sb = Sandbox::new("r01_unknown_sub");
    let cwd = sb.work_dir();
    for bad in ["delete", "rm", "remove", "nuke", "shred", "wipe"] {
        let res = run_cli(&cwd, &[bad, "--format", "json"]);
        assert_eq!(res.exit_code, 1, "subcommand `{}` must fail with exit 1", bad);
        let j = res.json();
        assert_eq!(j["error"]["code"], "E_UNKNOWN_ARG");
    }
}

#[test]
fn help_explicitly_states_never_permanently_deletes() {
    let sb = Sandbox::new("r01_help");
    let res = run_cli(&sb.work_dir(), &["--help"]);
    assert_eq!(res.exit_code, 0);
    assert!(
        res.stdout.contains("永不永久删除"),
        "help text must state 永不永久删除:\n{}",
        res.stdout
    );
    assert!(
        res.stdout.contains("永不清空回收站"),
        "help text must state 永不清空回收站:\n{}",
        res.stdout
    );
}

#[test]
fn positive_put_sends_file_to_recycle_bin() {
    let sb = Sandbox::new("r01_positive");
    let file = data::plain_file(&sb.data_dir(), "hello.txt", "safe recycle");
    let arg = sb.checked_arg(&file);

    let res = run_cli(&sb.work_dir(), &["put", "--format", "json", &arg]);
    assert_eq!(res.exit_code, 0, "stdout={} stderr={}", res.stdout, res.stderr);
    assert!(!file.exists(), "file should be moved to recycle bin");
    let j = res.json();
    assert_eq!(j["items"][0]["outcome"], "succeeded");
    assert_eq!(j["items"][0]["code"], "E_OK");
    assert!(j["items"][0]["trash_id"].as_str().map(|s| !s.is_empty()).unwrap_or(false));
}

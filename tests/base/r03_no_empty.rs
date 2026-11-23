mod support;

use support::data;
use support::probe::all_src_rs_files;
use support::run::run_cli;
use support::sandbox::Sandbox;

#[test]
fn static_no_empty_recycle_bin_api() {
    for file in all_src_rs_files() {
        let content = std::fs::read_to_string(&file).expect("read src file");
        assert!(
            !content.contains("SHEmptyRecycleBin"),
            "R3 violation: SHEmptyRecycleBin found in {}",
            file.display()
        );
    }
}

#[test]
fn rejects_all_empty_commands_and_flags() {
    let sb = Sandbox::new("r03_no_empty");
    let cwd = sb.work_dir();
    for cmd in ["empty", "--empty", "clear", "purge", "trash-empty"] {
        let res = run_cli(&cwd, &[cmd, "--format", "json"]);
        assert_eq!(res.exit_code, 1, "`{}` must be rejected", cmd);
        let j = res.json();
        assert_eq!(j["error"]["code"], "E_UNKNOWN_ARG");
    }

    let file = data::plain_file(&sb.data_dir(), "keep.txt", "keep");
    let arg = sb.checked_arg(&file);
    let res = run_cli(&cwd, &["put", "--empty", "--format", "json", &arg]);
    assert_eq!(res.exit_code, 1);
    assert_eq!(res.json()["error"]["code"], "E_UNKNOWN_ARG");
    assert!(file.exists(), "file must remain untouched");
}

#[test]
fn recycle_bin_count_is_non_decreasing() {
    let sb = Sandbox::new("r03_non_decreasing");
    let cwd = sb.work_dir();
    let before_res = run_cli(&cwd, &["list", "--format", "json"]);
    assert_eq!(before_res.exit_code, 0);
    let before_count = before_res.json()["summary"]["total"].as_u64().unwrap();

    let file = data::plain_file(&sb.data_dir(), "r03_inc.txt", "count me");
    let arg = sb.checked_arg(&file);
    let put_res = run_cli(&cwd, &["put", "--format", "json", &arg]);
    assert_eq!(put_res.exit_code, 0);

    let after_res = run_cli(&cwd, &["list", "--format", "json"]);
    assert_eq!(after_res.exit_code, 0);
    let after_count = after_res.json()["summary"]["total"].as_u64().unwrap();
    assert!(
        after_count >= before_count + 1,
        "recycle bin count must not decrease: before={}, after={}",
        before_count,
        after_count
    );
}

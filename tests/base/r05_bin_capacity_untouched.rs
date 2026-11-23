mod support;

use support::data;
use support::probe::all_src_rs_files;
use support::run::run_cli;
use support::sandbox::Sandbox;

#[test]
fn static_no_recycle_bin_capacity_or_policy_queries() {
    let forbidden = [
        "MaxCapacity",
        "NukeOnDelete",
        "SHQueryRecycleBin",
        "BitBucket",
    ];
    for file in all_src_rs_files() {
        let content = std::fs::read_to_string(&file).expect("read src file");
        for pat in forbidden {
            assert!(
                !content.contains(pat),
                "R5 violation: forbidden capacity/policy symbol `{}` found in {}",
                pat,
                file.display()
            );
        }
    }
}

#[test]
fn runtime_no_capacity_hints_in_output() {
    let sb = Sandbox::new("r05_no_capacity_hints");
    let cwd = sb.work_dir();
    let f = data::plain_file(&sb.data_dir(), "cap_check.txt", "hello");
    let arg = sb.checked_arg(&f);

    let res = run_cli(&cwd, &["put", &arg]);
    assert_eq!(res.exit_code, 0);
    for forbidden_hint in ["容量", "已满", "清理回收站", "清空"] {
        assert!(
            !res.stdout.contains(forbidden_hint),
            "R5 violation: unexpected capacity hint `{}` in stdout:\n{}",
            forbidden_hint,
            res.stdout
        );
    }
}

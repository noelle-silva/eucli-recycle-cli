mod support;

use support::data;
use support::run::run_cli;
use support::sandbox::Sandbox;

#[test]
fn rejects_all_rm_style_and_unknown_flags() {
    let sb = Sandbox::new("r14_reject_rm");
    let dd = sb.data_dir();
    let cwd = sb.work_dir();
    let f = data::plain_file(&dd, "keep.txt", "stay");
    let arg = sb.checked_arg(&f);

    for bad_flag in [
        "-f",
        "-rf",
        "-r",
        "-i",
        "-v",
        "--force",
        "--recursive",
        "--interactive",
        "--verbose",
        "--unknown-flag",
    ] {
        let res = run_cli(&cwd, &["put", bad_flag, "--format", "json", &arg]);
        assert_eq!(
            res.exit_code, 1,
            "flag `{}` must be rejected with exit 1, stdout={}",
            bad_flag, res.stdout
        );
        assert_eq!(res.json()["error"]["code"], "E_UNKNOWN_ARG");
    }

    assert!(f.exists(), "file must remain untouched");
}

#[test]
fn documented_options_genuinely_take_effect() {
    let sb = Sandbox::new("r14_real_opts");
    let dd = sb.data_dir();
    let cwd = sb.work_dir();

    // --help works
    let r_help = run_cli(&cwd, &["put", "--help"]);
    assert_eq!(r_help.exit_code, 0);
    assert!(r_help.stdout.contains("--dir"));

    // --format json vs human genuinely changes format
    let f = data::plain_file(&dd, "fmt.txt", "x");
    let arg = sb.checked_arg(&f);
    let r_dry_json = run_cli(&cwd, &["put", "--dry-run", "--format", "json", &arg]);
    assert_eq!(r_dry_json.exit_code, 0);
    assert!(serde_json::from_str::<serde_json::Value>(&r_dry_json.stdout).is_ok());

    let r_dry_human = run_cli(&cwd, &["put", "--dry-run", "--format", "human", &arg]);
    assert_eq!(r_dry_human.exit_code, 0);
    assert!(r_dry_human.stdout.contains("[计划执行]"));
}

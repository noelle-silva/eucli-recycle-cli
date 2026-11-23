mod support;

use support::data;
use support::run::run_cli;
use support::sandbox::Sandbox;

#[test]
fn long_path_dir_refused_and_left_intact_without_dialog() {
    let sb = Sandbox::new("r02_long_dir");
    let (top_dir, leaf_ext) = data::long_path(&sb.data_dir(), "too_long_dir");
    let arg = sb.checked_arg(&top_dir);

    let res = run_cli(&sb.work_dir(), &["put", "--dir", "--format", "json", &arg]);
    assert_eq!(
        res.exit_code, 3,
        "long-path directory must fail with exit 3, stdout={}",
        res.stdout
    );
    let j = res.json();
    assert_eq!(j["items"][0]["outcome"], "failed");
    assert_eq!(j["items"][0]["code"], "E_RECYCLE_REFUSED");
    assert!(top_dir.exists(), "R2 violation: top directory was deleted!");
    assert!(leaf_ext.exists(), "R2 violation: deep leaf file was deleted!");
}

#[test]
fn long_path_file_refused_and_left_intact() {
    let sb = Sandbox::new("r02_long_file");
    let (_top_dir, leaf_ext) = data::long_path(&sb.data_dir(), "too_long_file_root");
    let plain_long = leaf_ext
        .to_string_lossy()
        .trim_start_matches(r"\\?\")
        .to_string();
    sb.assert_inside(std::path::Path::new(&plain_long));

    let res = run_cli(&sb.work_dir(), &["put", "--format", "json", &plain_long]);
    assert_eq!(
        res.exit_code, 3,
        "direct >260 char file must fail with exit 3, stdout={}",
        res.stdout
    );
    let j = res.json();
    assert_eq!(j["items"][0]["outcome"], "failed");
    assert_eq!(j["items"][0]["code"], "E_RECYCLE_REFUSED");
    assert!(leaf_ext.exists(), "R2 violation: long-path file was deleted!");
}

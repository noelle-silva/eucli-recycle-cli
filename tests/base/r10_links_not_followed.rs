mod support;

use support::data;
use support::run::run_cli;
use support::sandbox::Sandbox;

#[test]
fn recycles_file_symlink_without_touching_target() {
    let sb = Sandbox::new("r10_sym_file");
    let dd = sb.data_dir();
    let cwd = sb.work_dir();

    let target = data::plain_file(&dd, "real_target.txt", "keep me safe");
    let link = data::symlink_file(&dd, "link_to_file.txt", &target)
        .expect("create symlink_file (requires Developer Mode or symlink privilege)");

    let res = run_cli(&cwd, &["put", "--format", "json", &sb.checked_arg(&link)]);
    assert_eq!(res.exit_code, 0, "stdout={}", res.stdout);
    assert!(link.symlink_metadata().is_err(), "symlink itself should be recycled");
    assert!(target.exists(), "R10 violation: target file must remain intact");
}

#[test]
fn recycles_dir_symlink_without_dir_flag_and_without_touching_target() {
    let sb = Sandbox::new("r10_sym_dir");
    let dd = sb.data_dir();
    let cwd = sb.work_dir();

    let target_dir = data::nested_dir(&dd, "real_dir");
    let link = data::symlink_dir(&dd, "link_to_dir", &target_dir)
        .expect("create symlink_dir");

    // Section 6.5: dir symlink is treated as a file/link, does NOT need --dir
    let res = run_cli(&cwd, &["put", "--format", "json", &sb.checked_arg(&link)]);
    assert_eq!(res.exit_code, 0, "stdout={}", res.stdout);
    assert!(link.symlink_metadata().is_err(), "dir symlink itself should be recycled");
    assert!(target_dir.exists(), "R10 violation: target dir must remain intact");
    assert!(target_dir.join("top.txt").exists(), "target dir contents must remain intact");
}

#[test]
fn recycles_junction_without_dir_flag_and_without_touching_target() {
    let sb = Sandbox::new("r10_junction");
    let dd = sb.data_dir();
    let cwd = sb.work_dir();

    let target_dir = data::nested_dir(&dd, "j_real_dir");
    let junction = data::junction_dir(&dd, "j_link", &target_dir);

    let res = run_cli(&cwd, &["put", "--format", "json", &sb.checked_arg(&junction)]);
    assert_eq!(res.exit_code, 0, "stdout={}", res.stdout);
    assert!(junction.symlink_metadata().is_err(), "junction itself should be recycled");
    assert!(target_dir.exists(), "R10 violation: junction target dir must remain intact");
    assert!(target_dir.join("top.txt").exists(), "junction target files must remain intact");
}

#[test]
fn recycles_dangling_symlink_cleanly() {
    let sb = Sandbox::new("r10_dangling");
    let dd = sb.data_dir();
    let cwd = sb.work_dir();

    let broken = data::dangling_link(&dd, "broken_link.txt")
        .expect("create dangling_link");

    let res = run_cli(&cwd, &["put", "--format", "json", &sb.checked_arg(&broken)]);
    assert_eq!(res.exit_code, 0, "stdout={}", res.stdout);
    assert!(broken.symlink_metadata().is_err(), "dangling link should be recycled");
}

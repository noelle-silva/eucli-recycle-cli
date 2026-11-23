mod support;

use support::data;
use support::run::run_cli;
use support::sandbox::Sandbox;

#[test]
fn directory_without_dir_flag_is_blocked() {
    let sb = Sandbox::new("r11_no_flag");
    let dir = data::nested_dir(&sb.data_dir(), "my_folder");
    let arg = sb.checked_arg(&dir);

    let res = run_cli(&sb.work_dir(), &["put", "--format", "json", &arg]);
    assert_eq!(res.exit_code, 2, "stdout={}", res.stdout);
    let j = res.json();
    assert_eq!(j["items"][0]["outcome"], "blocked");
    assert_eq!(j["items"][0]["code"], "E_NEEDS_DIR_FLAG");
    assert!(
        j["items"][0]["message"].as_str().unwrap_or("").contains("--dir"),
        "error message must explain how to declare --dir: {}",
        res.stdout
    );
    assert!(dir.exists(), "directory must remain untouched");
}

#[test]
fn directory_with_dir_flag_succeeds() {
    let sb = Sandbox::new("r11_with_flag");
    let dir = data::nested_dir(&sb.data_dir(), "removable_folder");
    let arg = sb.checked_arg(&dir);

    let res = run_cli(&sb.work_dir(), &["put", "--dir", "--format", "json", &arg]);
    assert_eq!(res.exit_code, 0, "stdout={}", res.stdout);
    assert!(!dir.exists());
    assert_eq!(res.json()["items"][0]["outcome"], "succeeded");
}

#[test]
fn file_with_dir_flag_is_blocked() {
    let sb = Sandbox::new("r11_file_with_dir");
    let file = data::plain_file(&sb.data_dir(), "just_a_file.txt", "content");
    let arg = sb.checked_arg(&file);

    let res = run_cli(&sb.work_dir(), &["put", "--dir", "--format", "json", &arg]);
    assert_eq!(res.exit_code, 2, "stdout={}", res.stdout);
    let j = res.json();
    assert_eq!(j["items"][0]["outcome"], "blocked");
    assert_eq!(j["items"][0]["code"], "E_DIR_FLAG_ON_FILE");
    assert!(file.exists(), "file must remain untouched");
}

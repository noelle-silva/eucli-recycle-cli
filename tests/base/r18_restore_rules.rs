mod support;

use std::fs;
use support::data;
use support::run::run_cli;
use support::sandbox::Sandbox;

#[test]
fn restores_recycled_file_and_directory_accurately() {
    let sb = Sandbox::new("r18_restore_ok");
    let dd = sb.data_dir();
    let cwd = sb.work_dir();

    // 1. File roundtrip
    let f = data::plain_file(&dd, "roundtrip.txt", "precious payload 42");
    let arg_f = sb.checked_arg(&f);
    let put_f = run_cli(&cwd, &["put", "--format", "json", &arg_f]);
    assert_eq!(put_f.exit_code, 0, "stdout={}", put_f.stdout);
    let id_f = put_f.json()["items"][0]["trash_id"].as_str().unwrap().to_string();
    assert!(!f.exists());

    let rest_f = run_cli(&cwd, &["restore", "--format", "json", &id_f]);
    assert_eq!(rest_f.exit_code, 0, "stdout={}", rest_f.stdout);
    let rj = rest_f.json();
    assert_eq!(rj["items"][0]["outcome"], "succeeded");
    assert_eq!(rj["items"][0]["code"], "E_OK");
    assert_eq!(rj["items"][0]["verified"]["present_at_original"], true);
    assert_eq!(rj["items"][0]["verified"]["absent_in_bin"], true);
    assert_eq!(fs::read_to_string(&f).unwrap(), "precious payload 42");

    // 2. Directory roundtrip
    let d = data::nested_dir(&dd, "roundtrip_dir");
    let arg_d = sb.checked_arg(&d);
    let put_d = run_cli(&cwd, &["put", "--dir", "--format", "json", &arg_d]);
    assert_eq!(put_d.exit_code, 0, "stdout={}", put_d.stdout);
    let id_d = put_d.json()["items"][0]["trash_id"].as_str().unwrap().to_string();
    assert!(!d.exists());

    let rest_d = run_cli(&cwd, &["restore", "--format", "json", &id_d]);
    assert_eq!(rest_d.exit_code, 0, "stdout={}", rest_d.stdout);
    assert!(d.join("level1").join("level2").join("level3").join("bottom.txt").exists());
}

#[test]
fn rejects_missing_trash_id() {
    let sb = Sandbox::new("r18_missing_id");
    let res = run_cli(
        &sb.work_dir(),
        &["restore", "--format", "json", "deadbeefcafebabe"],
    );
    assert_eq!(res.exit_code, 2, "stdout={}", res.stdout);
    let j = res.json();
    assert_eq!(j["items"][0]["outcome"], "blocked");
    assert_eq!(j["items"][0]["code"], "E_TRASH_ID_NOT_FOUND");
}

#[test]
fn rejects_restore_when_origin_parent_missing() {
    let sb = Sandbox::new("r18_parent_missing");
    let dd = sb.data_dir();
    let cwd = sb.work_dir();

    let sub = data::empty_dir(&dd, "ephemeral_parent");
    let f = data::plain_file(&sub, "orphan.txt", "orphan");
    let arg = sb.checked_arg(&f);

    let put = run_cli(&cwd, &["put", "--format", "json", &arg]);
    assert_eq!(put.exit_code, 0);
    let id = put.json()["items"][0]["trash_id"].as_str().unwrap().to_string();

    // Remove the now-empty parent directory inside our sandbox
    fs::remove_dir(&sub).unwrap();

    let rest = run_cli(&cwd, &["restore", "--format", "json", &id]);
    assert_eq!(rest.exit_code, 2, "stdout={}", rest.stdout);
    assert_eq!(rest.json()["items"][0]["outcome"], "blocked");
    assert_eq!(rest.json()["items"][0]["code"], "E_ORIGIN_DIR_MISSING");
    assert!(!sub.exists(), "restore must NOT recreate missing parent directory");
}

#[test]
fn rejects_restore_when_origin_occupied_and_never_overwrites() {
    let sb = Sandbox::new("r18_occupied");
    let dd = sb.data_dir();
    let cwd = sb.work_dir();

    let f = data::plain_file(&dd, "collide.txt", "old version");
    let arg = sb.checked_arg(&f);

    let put = run_cli(&cwd, &["put", "--format", "json", &arg]);
    assert_eq!(put.exit_code, 0);
    let id = put.json()["items"][0]["trash_id"].as_str().unwrap().to_string();

    // Place a new file at the exact same original path
    fs::write(&f, b"new occupant do not overwrite").unwrap();

    let rest = run_cli(&cwd, &["restore", "--format", "json", &id]);
    assert_eq!(rest.exit_code, 2, "stdout={}", rest.stdout);
    assert_eq!(rest.json()["items"][0]["outcome"], "blocked");
    assert_eq!(rest.json()["items"][0]["code"], "E_ORIGIN_OCCUPIED");
    assert_eq!(
        fs::read_to_string(&f).unwrap(),
        "new occupant do not overwrite",
        "R18 violation: occupying file was overwritten!"
    );
}

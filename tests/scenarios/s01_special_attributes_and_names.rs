#[path = "../base/support/mod.rs"]
mod support;

use std::fs;
use std::time::Duration;
use support::data;
use support::run::{run_cli, run_cli_with_timeout};
use support::sandbox::Sandbox;

const BIN_SCAN_TIMEOUT: Duration = Duration::from_secs(120);

#[test]
fn readonly_file_roundtrip_put_list_restore() {
    let sb = Sandbox::new("s01_readonly");
    let dd = sb.data_dir();
    let cwd = sb.work_dir();

    let ro = data::readonly_file(&dd, "readonly_doc.txt");
    let arg = sb.checked_arg(&ro);

    let put = run_cli(&cwd, &["put", "--format", "json", &arg]);
    assert_eq!(put.exit_code, 0, "stdout={} stderr={}", put.stdout, put.stderr);
    assert!(!ro.exists(), "readonly file should be recycled");
    let tid = put.json()["items"][0]["trash_id"].as_str().unwrap().to_string();

    let dd_s = dd.to_string_lossy().to_string();
    let list = run_cli_with_timeout(
        &cwd,
        &["list", "--from", &dd_s, "--name", "readonly_doc.txt", "--format", "json"],
        BIN_SCAN_TIMEOUT,
    );
    assert_eq!(list.exit_code, 0);
    assert!(
        list.json()["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|it| it["trash_id"].as_str() == Some(tid.as_str())),
        "readonly file trash_id must appear in list"
    );

    let rest = run_cli_with_timeout(&cwd, &["restore", "--format", "json", &tid], BIN_SCAN_TIMEOUT);
    assert_eq!(rest.exit_code, 0, "stdout={}", rest.stdout);
    assert_eq!(fs::read_to_string(&ro).unwrap(), "readonly content");
}

#[test]
fn hidden_system_file_roundtrip_put_and_restore() {
    let sb = Sandbox::new("s01_hidden_sys");
    let dd = sb.data_dir();
    let cwd = sb.work_dir();

    let hs = data::hidden_system_file(&dd, "secret_sys.dat");
    let arg = sb.checked_arg(&hs);

    let put = run_cli(&cwd, &["put", "--format", "json", &arg]);
    assert_eq!(put.exit_code, 0, "stdout={} stderr={}", put.stdout, put.stderr);
    assert!(hs.symlink_metadata().is_err(), "hidden+system file should be recycled");
    let tid = put.json()["items"][0]["trash_id"].as_str().unwrap().to_string();

    let rest = run_cli_with_timeout(&cwd, &["restore", "--format", "json", &tid], BIN_SCAN_TIMEOUT);
    assert_eq!(rest.exit_code, 0, "stdout={}", rest.stdout);
    assert_eq!(fs::read_to_string(&hs).unwrap(), "hidden system content");
}

#[test]
fn weird_names_batch_put_list_and_selective_restore() {
    let sb = Sandbox::new("s01_weird_roundtrip");
    let dd = sb.data_dir();
    let cwd = sb.work_dir();

    let files = data::weird_names(&dd);
    let owned: Vec<String> = files.iter().map(|p| sb.checked_arg(p)).collect();
    let mut args = vec!["put", "--format", "json"];
    for s in &owned {
        args.push(s.as_str());
    }

    let put = run_cli(&cwd, &args);
    assert_eq!(put.exit_code, 0, "stdout={} stderr={}", put.stdout, put.stderr);
    let pj = put.json();
    let items = pj["items"].as_array().unwrap();
    assert_eq!(items.len(), files.len());

    // Restore Chinese, emoji, and leading-dash files back to original paths
    let mut ids_to_restore = Vec::new();
    let mut expected_paths = Vec::new();
    for (idx, path) in files.iter().enumerate() {
        let name = path.file_name().unwrap().to_string_lossy();
        if name == "中文文件.txt" || name == "emoji_🚀_test.txt" || name == "-leading-dash.txt" {
            ids_to_restore.push(items[idx]["trash_id"].as_str().unwrap().to_string());
            expected_paths.push(path.clone());
        }
    }
    assert_eq!(ids_to_restore.len(), 3);

    let mut rest_args = vec!["restore", "--format", "json"];
    for id in &ids_to_restore {
        rest_args.push(id.as_str());
    }
    let rest = run_cli_with_timeout(&cwd, &rest_args, BIN_SCAN_TIMEOUT);
    assert_eq!(rest.exit_code, 0, "stdout={}", rest.stdout);
    for p in expected_paths {
        assert!(p.exists(), "weird-named file `{}` must be restored", p.display());
        let name = p.file_name().unwrap().to_string_lossy();
        assert_eq!(fs::read_to_string(&p).unwrap(), format!("content of {}", name));
    }
}

#[test]
fn mixed_slashes_and_case_on_unicode_directory_with_spaces() {
    let sb = Sandbox::new("s01_unicode_dir");
    let dd = sb.data_dir();
    let cwd = sb.work_dir();

    let dir = data::nested_dir(&dd, "测试 目录 (2026)");
    sb.assert_inside(&dir);
    let messy_input = format!("{}/", dir.to_string_lossy().replace('\\', "/"));

    let put = run_cli(&cwd, &["put", "--dir", "--format", "json", &messy_input]);
    assert_eq!(put.exit_code, 0, "stdout={}", put.stdout);
    assert!(!dir.exists());
    let tid = put.json()["items"][0]["trash_id"].as_str().unwrap().to_string();

    let rest = run_cli_with_timeout(&cwd, &["restore", "--format", "json", &tid], BIN_SCAN_TIMEOUT);
    assert_eq!(rest.exit_code, 0, "stdout={}", rest.stdout);
    assert!(dir.join("level1").join("level2").join("level3").join("bottom.txt").exists());
}

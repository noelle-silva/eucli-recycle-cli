#[path = "../base/support/mod.rs"]
mod support;

use std::fs;
use std::path::Path;
use std::time::Duration;
use support::data;
use support::run::{run_cli, run_cli_with_timeout};
use support::sandbox::Sandbox;

const BIN_SCAN_TIMEOUT: Duration = Duration::from_secs(120);

#[test]
fn cross_volume_d_drive_file_and_dir_roundtrip_if_available() {
    let d_root = Path::new(r"D:\");
    if !d_root.exists() {
        return;
    }
    let d_base = d_root.join("eucli-recycle-test-sandboxes");
    if fs::create_dir_all(&d_base).is_err() {
        return;
    }

    let sb = Sandbox::new_under(&d_base, "s07_d_drive");
    let dd = sb.data_dir();
    let cwd = sb.work_dir();

    // 1. File on D: put -> list -> restore
    let f = data::plain_file(&dd, "d_file.txt", "hello from D drive");
    let arg_f = sb.checked_arg(&f);
    let put_f = run_cli(&cwd, &["put", "--format", "json", &arg_f]);
    assert_eq!(put_f.exit_code, 0, "stdout={} stderr={}", put_f.stdout, put_f.stderr);
    assert!(!f.exists());
    let id_f = put_f.json()["items"][0]["trash_id"].as_str().unwrap().to_string();

    let dd_s = dd.to_string_lossy().to_string();
    let list_f = run_cli_with_timeout(
        &cwd,
        &["list", "--from", &dd_s, "--name", "d_file.txt", "--format", "json"],
        BIN_SCAN_TIMEOUT,
    );
    assert_eq!(list_f.exit_code, 0);
    assert!(
        list_f.json()["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|it| it["trash_id"].as_str() == Some(id_f.as_str())),
        "D: file must be found in list: {}",
        list_f.stdout
    );

    let rest_f = run_cli_with_timeout(&cwd, &["restore", "--format", "json", &id_f], BIN_SCAN_TIMEOUT);
    assert_eq!(rest_f.exit_code, 0, "stdout={}", rest_f.stdout);
    assert_eq!(fs::read_to_string(&f).unwrap(), "hello from D drive");

    // 2. Directory on D: put --dir -> restore
    let d = data::nested_dir(&dd, "d_nested_dir");
    let arg_d = sb.checked_arg(&d);
    let put_d = run_cli(&cwd, &["put", "--dir", "--format", "json", &arg_d]);
    assert_eq!(put_d.exit_code, 0, "stdout={}", put_d.stdout);
    assert!(!d.exists());
    let id_d = put_d.json()["items"][0]["trash_id"].as_str().unwrap().to_string();

    let rest_d = run_cli_with_timeout(&cwd, &["restore", "--format", "json", &id_d], BIN_SCAN_TIMEOUT);
    assert_eq!(rest_d.exit_code, 0, "stdout={}", rest_d.stdout);
    assert!(d.join("level1").join("level2").join("level3").join("bottom.txt").exists());

    drop(sb);
    let _ = fs::remove_dir(&d_base); // removes base folder if empty
}

#[test]
fn recycling_directory_containing_symlinks_never_touches_external_targets() {
    let sb = Sandbox::new("s07_dir_with_links");
    let dd = sb.data_dir();
    let cwd = sb.work_dir();

    // External targets that must remain untouched
    let ext_file = data::plain_file(&dd, "precious_external.txt", "never touch");
    let ext_dir = data::nested_dir(&dd, "precious_ext_dir");

    // Directory to recycle, containing both a file symlink and a junction pointing outside
    let container = data::empty_dir(&dd, "container_with_links");
    data::plain_file(&container, "normal.txt", "normal inside");
    let _fl = data::symlink_file(&container, "link_to_ext.txt", &ext_file)
        .expect("create symlink_file inside container");
    let _jl = data::junction_dir(&container, "junc_to_ext_dir", &ext_dir);

    let arg = sb.checked_arg(&container);
    let put = run_cli(&cwd, &["put", "--dir", "--format", "json", &arg]);
    assert_eq!(put.exit_code, 0, "stdout={} stderr={}", put.stdout, put.stderr);
    assert!(!container.exists(), "container dir should be recycled");

    assert_eq!(
        fs::read_to_string(&ext_file).unwrap(),
        "never touch",
        "external file linked from inside recycled dir must remain intact"
    );
    assert!(
        ext_dir.join("level1").join("level2").join("level3").join("bottom.txt").exists(),
        "external dir linked via junction inside recycled dir must remain intact"
    );
}

#[test]
fn symlink_pointing_into_protected_tree_can_be_recycled_without_affecting_protected_dir() {
    let sb = Sandbox::new("s07_link_to_protected");
    let dd = sb.data_dir();
    let cwd = sb.work_dir();

    let protected = data::nested_dir(&dd, "protected_real_dir");
    let link = data::symlink_dir(&dd, "shortcut_to_protected", &protected)
        .expect("create symlink_dir");

    let cfg = sb.path().join("prot.toml");
    let cfg_body = format!(
        "schema = 1\n\n[protect.tree]\npaths = [\"{}\"]\n",
        protected.to_string_lossy().replace('\\', r"\\")
    );
    fs::write(&cfg, cfg_body).unwrap();
    let cfg_s = cfg.to_string_lossy().to_string();

    // Link itself is NOT inside protected tree; recycling the link removes only the link
    let put = run_cli(
        &cwd,
        &["put", "--config", &cfg_s, "--format", "json", &sb.checked_arg(&link)],
    );
    assert_eq!(put.exit_code, 0, "stdout={}", put.stdout);
    assert!(link.symlink_metadata().is_err(), "symlink itself should be recycled");
    assert!(
        protected.join("top.txt").exists(),
        "protected target directory must remain completely untouched"
    );
}

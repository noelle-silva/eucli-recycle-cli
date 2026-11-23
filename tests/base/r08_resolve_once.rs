mod support;

use std::fs;
use support::data;
use support::sandbox::Sandbox;

#[test]
fn detects_target_replaced_with_symlink_between_check_and_execute() {
    let sb = Sandbox::new("r08_swap_symlink");
    let dd = sb.data_dir();
    let cwd = sb.work_dir();

    let target = data::plain_file(&dd, "victim.txt", "original");
    let decoy = data::plain_file(&dd, "decoy_target.txt", "do not touch me");
    sb.assert_inside(&target);

    let target_clone = target.clone();
    let decoy_clone = decoy.clone();

    // Run internal put pipeline with a pre-execute hook that replaces the checked file
    // with a symlink pointing to `decoy` (R8 TOCTOU test).
    let report = eucli_recycle::run_put_with_pre_exec_hook(
        &cwd,
        &[target.to_string_lossy().to_string()],
        false,
        false,
        None,
        Some(Box::new(move || {
            let _ = fs::remove_file(&target_clone);
            std::os::windows::fs::symlink_file(&decoy_clone, &target_clone)
                .expect("replace target with symlink");
        })),
    );

    assert_eq!(report.exit_code, 3, "report={:?}", report);
    assert_eq!(report.items.len(), 1);
    assert_eq!(report.items[0].outcome.as_str(), "failed");
    assert_eq!(report.items[0].code.as_str(), "E_TARGET_CHANGED");
    assert!(
        target.symlink_metadata().is_ok(),
        "R8 violation: replaced symlink must not be touched"
    );
    assert!(
        decoy.exists(),
        "R8 violation: decoy target must remain intact"
    );
}

#[test]
fn detects_target_replaced_with_new_file_between_check_and_execute() {
    let sb = Sandbox::new("r08_swap_file");
    let dd = sb.data_dir();
    let cwd = sb.work_dir();

    let target = data::plain_file(&dd, "swap_me.txt", "v1");
    sb.assert_inside(&target);
    let target_clone = target.clone();
    let dir_clone = dd.clone();

    let report = eucli_recycle::run_put_with_pre_exec_hook(
        &cwd,
        &[target.to_string_lossy().to_string()],
        false,
        false,
        None,
        Some(Box::new(move || {
            // Create a brand new file on a fresh inode/file-index and move it onto target
            let other = dir_clone.join("fresh_inode.txt");
            fs::write(&other, b"brand new content").unwrap();
            fs::remove_file(&target_clone).unwrap();
            fs::rename(&other, &target_clone).unwrap();
        })),
    );

    assert_eq!(report.exit_code, 3, "report={:?}", report);
    assert_eq!(report.items[0].outcome.as_str(), "failed");
    assert_eq!(report.items[0].code.as_str(), "E_TARGET_CHANGED");
    assert_eq!(fs::read_to_string(&target).unwrap(), "brand new content");
}

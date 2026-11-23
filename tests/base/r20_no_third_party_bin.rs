mod support;

use std::fs;
use support::probe::{all_src_rs_files, workspace_root};

#[test]
fn cargo_toml_has_no_third_party_trash_dependencies() {
    let cargo_toml = fs::read_to_string(workspace_root().join("Cargo.toml"))
        .expect("read Cargo.toml");
    let parsed: toml::Value = toml::from_str(&cargo_toml).expect("parse Cargo.toml");

    let forbidden_crates = ["trash", "trash-rs", "trash-cli", "trash_cli", "winapi-trash"];

    let check_table = |table: Option<&toml::map::Map<String, toml::Value>>| {
        if let Some(deps) = table {
            for dep_name in deps.keys() {
                for bad in forbidden_crates {
                    assert!(
                        !dep_name.eq_ignore_ascii_case(bad),
                        "R20 violation: forbidden third-party recycle crate `{}` in Cargo.toml",
                        dep_name
                    );
                }
            }
        }
    };

    check_table(parsed.get("dependencies").and_then(|v| v.as_table()));
    check_table(parsed.get("dev-dependencies").and_then(|v| v.as_table()));
    if let Some(targets) = parsed.get("target").and_then(|v| v.as_table()) {
        for target_cfg in targets.values() {
            check_table(target_cfg.get("dependencies").and_then(|v| v.as_table()));
        }
    }
}

#[test]
fn platform_layer_only_uses_official_windows_crate() {
    let platform_dir = workspace_root().join("src").join("platform");
    let platform_files: Vec<_> = all_src_rs_files()
        .into_iter()
        .filter(|p| p.starts_with(&platform_dir))
        .collect();
    assert!(
        !platform_files.is_empty(),
        "platform layer source files must exist"
    );
    for file in platform_files {
        let content = fs::read_to_string(&file).expect("read platform file");
        assert!(
            !content.contains("use trash") && !content.contains("trash::"),
            "R20 violation: third-party trash usage in {}",
            file.display()
        );
    }
}

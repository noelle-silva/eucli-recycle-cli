mod support;

use std::fs;
use support::data;
use support::run::run_cli;
use support::sandbox::Sandbox;

#[test]
fn blocks_builtin_hard_protections() {
    let sb = Sandbox::new("r09_builtin");
    let cwd = sb.work_dir();

    // Drive root -> E_DRIVE_ROOT
    let r_root = run_cli(&cwd, &["put", "--dir", "--format", "json", r"E:\"]);
    assert_eq!(r_root.exit_code, 2);
    assert_eq!(r_root.json()["items"][0]["code"], "E_DRIVE_ROOT");

    // System dirs -> E_BLACKLIST_TREE
    for sys_dir in [r"C:\Windows", r"C:\Program Files", r"C:\ProgramData"] {
        let r = run_cli(&cwd, &["put", "--dir", "--format", "json", sys_dir]);
        assert_eq!(r.exit_code, 2, "sys_dir={}", sys_dir);
        assert_eq!(r.json()["items"][0]["code"], "E_BLACKLIST_TREE");
    }

    // Descendant of tree-protected system dir -> E_PROTECTED_PARENT
    let r_sub = run_cli(&cwd, &["put", "--dir", "--format", "json", r"C:\Windows\System32"]);
    assert_eq!(r_sub.exit_code, 2);
    assert_eq!(r_sub.json()["items"][0]["code"], "E_PROTECTED_PARENT");

    // Parent containing protected user AppData (C:\Users) -> E_PROTECTED_PARENT
    let r_users = run_cli(&cwd, &["put", "--dir", "--format", "json", r"C:\Users"]);
    assert_eq!(r_users.exit_code, 2);
    assert_eq!(r_users.json()["items"][0]["code"], "E_PROTECTED_PARENT");
}

#[test]
fn enforces_self_and_tree_config_blacklist_and_containment() {
    let sb = Sandbox::new("r09_config_layers");
    let dd = sb.data_dir();
    let cwd = sb.work_dir();

    let self_dir = data::empty_dir(&dd, "prot_self");
    let self_child = data::plain_file(&self_dir, "allowed.txt", "can delete");
    let tree_parent = data::empty_dir(&dd, "tree_parent");
    let tree_dir = data::empty_dir(&tree_parent, "prot_tree");
    let tree_child = data::plain_file(&tree_dir, "forbidden.txt", "cannot delete");

    let cfg = sb.path().join("cfg.toml");
    let cfg_body = format!(
        "schema = 1\n\n[protect.self]\npaths = [\"{}\"]\n\n[protect.tree]\npaths = [\"{}\"]\n",
        self_dir.to_string_lossy().replace('\\', r"\\"),
        tree_dir.to_string_lossy().replace('\\', r"\\"),
    );
    fs::write(&cfg, cfg_body).unwrap();
    let cfg_s = cfg.to_string_lossy().to_string();

    // Exact hit on protect.self -> E_BLACKLIST_SELF
    let r1 = run_cli(
        &cwd,
        &["put", "--dir", "--config", &cfg_s, "--format", "json", &sb.checked_arg(&self_dir)],
    );
    assert_eq!(r1.exit_code, 2);
    assert_eq!(r1.json()["items"][0]["code"], "E_BLACKLIST_SELF");

    // Child inside protect.self -> ALLOWED
    let r2 = run_cli(
        &cwd,
        &["put", "--config", &cfg_s, "--format", "json", &sb.checked_arg(&self_child)],
    );
    assert_eq!(r2.exit_code, 0, "stdout={}", r2.stdout);
    assert!(!self_child.exists());

    // Exact hit on protect.tree -> E_BLACKLIST_TREE
    let r3 = run_cli(
        &cwd,
        &["put", "--dir", "--config", &cfg_s, "--format", "json", &sb.checked_arg(&tree_dir)],
    );
    assert_eq!(r3.exit_code, 2);
    assert_eq!(r3.json()["items"][0]["code"], "E_BLACKLIST_TREE");

    // Child inside protect.tree -> E_PROTECTED_PARENT
    let r4 = run_cli(
        &cwd,
        &["put", "--config", &cfg_s, "--format", "json", &sb.checked_arg(&tree_child)],
    );
    assert_eq!(r4.exit_code, 2);
    assert_eq!(r4.json()["items"][0]["code"], "E_PROTECTED_PARENT");
    assert!(tree_child.exists());

    // Ancestor containing protected object -> E_PROTECTED_PARENT
    let r5 = run_cli(
        &cwd,
        &["put", "--dir", "--config", &cfg_s, "--format", "json", &sb.checked_arg(&tree_parent)],
    );
    assert_eq!(r5.exit_code, 2);
    assert_eq!(r5.json()["items"][0]["code"], "E_PROTECTED_PARENT");
    assert!(tree_parent.exists());
}

#[test]
fn blocks_cwd_and_ancestors_but_allows_siblings_and_children() {
    let sb = Sandbox::new("r09_cwd");
    let dd = sb.data_dir();
    let parent = data::empty_dir(&dd, "parent");
    let current = data::empty_dir(&parent, "current");
    let sibling = data::empty_dir(&parent, "sibling");
    let child = data::plain_file(&current, "child.txt", "ok");

    // CWD itself -> E_CWD_ITSELF
    let r_self = run_cli(
        &current,
        &["put", "--dir", "--format", "json", &sb.checked_arg(&current)],
    );
    assert_eq!(r_self.exit_code, 2);
    assert_eq!(r_self.json()["items"][0]["code"], "E_CWD_ITSELF");
    assert!(current.exists());

    // Ancestor of CWD -> E_CWD_ANCESTOR
    let r_anc = run_cli(
        &current,
        &["put", "--dir", "--format", "json", &sb.checked_arg(&parent)],
    );
    assert_eq!(r_anc.exit_code, 2);
    assert_eq!(r_anc.json()["items"][0]["code"], "E_CWD_ANCESTOR");
    assert!(parent.exists());

    // Sibling -> ALLOWED
    let r_sib = run_cli(
        &current,
        &["put", "--dir", "--format", "json", &sb.checked_arg(&sibling)],
    );
    assert_eq!(r_sib.exit_code, 0, "stdout={}", r_sib.stdout);
    assert!(!sibling.exists());

    // Child inside CWD -> ALLOWED
    let r_child = run_cli(
        &current,
        &["put", "--format", "json", &sb.checked_arg(&child)],
    );
    assert_eq!(r_child.exit_code, 0, "stdout={}", r_child.stdout);
    assert!(!child.exists());
}

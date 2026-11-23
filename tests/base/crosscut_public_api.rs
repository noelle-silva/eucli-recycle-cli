mod support;

use support::run::run_cli;
use support::sandbox::Sandbox;

#[test]
fn version_and_help_entrypoints_are_consistent() {
    let sb = Sandbox::new("crosscut_api");
    let cwd = sb.work_dir();

    // --version
    let v = run_cli(&cwd, &["--version"]);
    assert_eq!(v.exit_code, 0);
    assert!(v.stdout.contains("eucli-recycle"));
    assert!(v.stdout.contains(env!("CARGO_PKG_VERSION")));

    // --help has all 4 required sections from Section 4.4
    let h = run_cli(&cwd, &["--help"]);
    assert_eq!(h.exit_code, 0);
    for required in [
        "能做什么",
        "永远不会做什么",
        "永不永久删除",
        "永不清空回收站",
        "示例",
        "可能遇到的错误",
    ] {
        assert!(
            h.stdout.contains(required),
            "top-level --help missing `{}`:\n{}",
            required,
            h.stdout
        );
    }

    // `help <cmd>` matches `<cmd> --help`
    for sub in ["put", "list", "restore"] {
        let a = run_cli(&cwd, &["help", sub]);
        let b = run_cli(&cwd, &[sub, "--help"]);
        assert_eq!(a.exit_code, 0);
        assert_eq!(b.exit_code, 0);
        assert_eq!(a.stdout, b.stdout, "help {} must match {} --help", sub, sub);
    }

    // No arguments -> exit 1 + E_NO_ARGS
    let empty = run_cli(&cwd, &["--format", "json"]);
    assert_eq!(empty.exit_code, 1);
    assert_eq!(empty.json()["error"]["code"], "E_NO_ARGS");

    // `put` with no targets -> exit 1 + E_NO_ARGS
    let put_empty = run_cli(&cwd, &["put", "--format", "json"]);
    assert_eq!(put_empty.exit_code, 1);
    assert_eq!(put_empty.json()["error"]["code"], "E_NO_ARGS");

    // `restore` with no ids -> exit 1 + E_NO_ARGS
    let rest_empty = run_cli(&cwd, &["restore", "--format", "json"]);
    assert_eq!(rest_empty.exit_code, 1);
    assert_eq!(rest_empty.json()["error"]["code"], "E_NO_ARGS");
}

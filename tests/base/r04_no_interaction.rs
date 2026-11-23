mod support;

use std::time::Duration;
use support::data;
use support::run::run_cli_with_timeout;
use support::sandbox::Sandbox;

#[test]
fn all_commands_finish_within_timeout_without_interaction() {
    let sb = Sandbox::new("r04_no_interaction");
    let cwd = sb.work_dir();
    let timeout = Duration::from_secs(30);

    // 1. help & version
    let r1 = run_cli_with_timeout(&cwd, &["--help"], timeout);
    assert_eq!(r1.exit_code, 0);
    let r2 = run_cli_with_timeout(&cwd, &["--version"], timeout);
    assert_eq!(r2.exit_code, 0);

    // 2. normal put
    let f = data::plain_file(&sb.data_dir(), "normal.txt", "ok");
    let arg = sb.checked_arg(&f);
    let r3 = run_cli_with_timeout(&cwd, &["put", "--format", "json", &arg], timeout);
    assert_eq!(r3.exit_code, 0);

    // 3. interactive-prone long path dir (historically popped up a GUI dialog)
    let (long_dir, _) = data::long_path(&sb.data_dir(), "no_dialog_long");
    let arg_long = sb.checked_arg(&long_dir);
    let r4 = run_cli_with_timeout(&cwd, &["put", "--dir", "--format", "json", &arg_long], timeout);
    assert_eq!(r4.exit_code, 3);

    // 4. list with limit
    let r5 = run_cli_with_timeout(&cwd, &["list", "--limit", "5", "--format", "json"], timeout);
    assert_eq!(r5.exit_code, 0);

    // 5. restore with nonexistent id
    let r6 = run_cli_with_timeout(
        &cwd,
        &["restore", "--format", "json", "0000000000000000"],
        timeout,
    );
    assert_eq!(r6.exit_code, 2);
}

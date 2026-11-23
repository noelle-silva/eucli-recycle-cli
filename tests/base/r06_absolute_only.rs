mod support;

use support::data;
use support::run::run_cli;
use support::sandbox::Sandbox;

#[test]
fn rejects_relative_and_non_drive_absolute_paths() {
    let sb = Sandbox::new("r06_reject");
    let dd = sb.data_dir();
    let f = data::plain_file(&dd, "stay.txt", "must stay");

    let cases = [
        ("", "E_EMPTY_PATH"),
        ("stay.txt", "E_NOT_ABSOLUTE"),
        (r"sub\stay.txt", "E_NOT_ABSOLUTE"),
        ("/e/work/stay.txt", "E_NOT_ABSOLUTE"),
        (r"\work\stay.txt", "E_NOT_ABSOLUTE"),
        ("E:stay.txt", "E_DRIVE_RELATIVE"),
        ("E:", "E_DRIVE_RELATIVE"),
    ];

    for (input, expected_code) in cases {
        let res = run_cli(&dd, &["put", "--format", "json", input]);
        assert_eq!(
            res.exit_code, 2,
            "input `{}` expected exit 2, got {} stdout={}",
            input, res.exit_code, res.stdout
        );
        let j = res.json();
        assert_eq!(j["items"][0]["outcome"], "blocked", "input `{}`", input);
        assert_eq!(
            j["items"][0]["code"], expected_code,
            "input `{}` wrong code: {}",
            input, res.stdout
        );
    }

    assert!(f.exists(), "target file must remain untouched");
}

#[test]
fn accepts_and_normalizes_valid_absolute_forms() {
    let sb = Sandbox::new("r06_accept");
    let dd = sb.data_dir();
    let cwd = sb.work_dir();

    // Forward slashes
    let f1 = data::plain_file(&dd, "fwd.txt", "1");
    sb.assert_inside(&f1);
    let fwd = f1.to_string_lossy().replace('\\', "/");
    let r1 = run_cli(&cwd, &["put", "--format", "json", &fwd]);
    assert_eq!(r1.exit_code, 0, "stdout={}", r1.stdout);
    assert!(!f1.exists());

    // Duplicate separators
    let f2 = data::plain_file(&dd, "dupsep.txt", "2");
    sb.assert_inside(&f2);
    let dup = f2.to_string_lossy().replace('\\', r"\\");
    let r2 = run_cli(&cwd, &["put", "--format", "json", &dup]);
    assert_eq!(r2.exit_code, 0, "stdout={}", r2.stdout);
    assert!(!f2.exists());

    // Trailing separator on non-root directory
    let d3 = data::empty_dir(&dd, "trail_dir");
    sb.assert_inside(&d3);
    let trail = format!(r"{}\", d3.display());
    let r3 = run_cli(&cwd, &["put", "--dir", "--format", "json", &trail]);
    assert_eq!(r3.exit_code, 0, "stdout={}", r3.stdout);
    assert!(!d3.exists());
}

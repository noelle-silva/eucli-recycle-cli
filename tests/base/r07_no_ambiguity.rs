mod support;

use std::path::Path;
use support::probe::probe_short_path;
use support::run::run_cli;
use support::sandbox::Sandbox;

#[test]
fn rejects_all_twelve_ambiguous_forms_and_reserved_names() {
    let sb = Sandbox::new("r07_ambiguity");
    let cwd = sb.work_dir();

    let cases = [
        // 1. empty
        ("", "E_EMPTY_PATH"),
        // 2. no drive letter
        (r"work\a.txt", "E_NOT_ABSOLUTE"),
        ("/e/work/a.txt", "E_NOT_ABSOLUTE"),
        (".", "E_NOT_ABSOLUTE"),
        ("..", "E_NOT_ABSOLUTE"),
        // 3. drive relative
        (r"E:work\a.txt", "E_DRIVE_RELATIVE"),
        ("E:", "E_DRIVE_RELATIVE"),
        // 4. leading backslash without drive
        (r"\work\a.txt", "E_NOT_ABSOLUTE"),
        // 5. device path
        (r"\\?\E:\a", "E_DEVICE_PATH"),
        (r"\\.\E:\a", "E_DEVICE_PATH"),
        // 6. UNC path
        (r"\\server\share\a", "E_UNC_UNSUPPORTED"),
        // 7. dot / dotdot segments
        (r"E:\dev\.\a", "E_DOT_COMPONENT"),
        (r"E:\dev\..", "E_DOT_COMPONENT"),
        // 8. trailing dot or space in a segment
        (r"E:\dev.", "E_TRAILING_DOT_SPACE"),
        (r"E:\dev ", "E_TRAILING_DOT_SPACE"),
        (r"E:\dev.\child.txt", "E_TRAILING_DOT_SPACE"),
        // 9. alternate data stream colon
        (r"E:\a.txt:stream", "E_ALT_DATA_STREAM"),
        // 10. wildcards
        (r"E:\a*.txt", "E_WILDCARD"),
        (r"E:\a?.txt", "E_WILDCARD"),
        // 11. 8.3 short name
        (r"C:\PROGRA~1", "E_SHORT_NAME"),
        (r"E:\sandbox\LONGFI~1.TXT", "E_SHORT_NAME"),
        // 12. invalid drive letter
        (r"1:\a", "E_BAD_DRIVE_LETTER"),
        // Reserved device names (6.3)
        (r"E:\sandbox\CON", "E_RESERVED_NAME"),
        (r"E:\sandbox\NUL.txt", "E_RESERVED_NAME"),
        (r"E:\sandbox\COM1", "E_RESERVED_NAME"),
        (r"E:\sandbox\LPT9.log", "E_RESERVED_NAME"),
    ];

    for (input, expected_code) in cases {
        let res = run_cli(&cwd, &["put", "--format", "json", input]);
        assert_eq!(
            res.exit_code, 2,
            "input `{}` should exit 2, got {} stdout={}",
            input, res.exit_code, res.stdout
        );
        let j = res.json();
        assert_eq!(j["items"][0]["outcome"], "blocked", "input `{}`", input);
        assert_eq!(
            j["items"][0]["code"], expected_code,
            "input `{}` expected {}, got {}",
            input, expected_code, j["items"][0]["code"]
        );
    }
}

#[test]
fn rejects_probed_real_83_short_name_on_c_drive() {
    let sb = Sandbox::new("r07_short_probe");
    let cwd = sb.work_dir();
    let pf = Path::new(r"C:\Program Files");
    if let Some(short) = probe_short_path(pf) {
        let res = run_cli(&cwd, &["put", "--dir", "--format", "json", &short]);
        assert_eq!(res.exit_code, 2);
        let j = res.json();
        assert_eq!(j["items"][0]["outcome"], "blocked");
        assert_eq!(j["items"][0]["code"], "E_SHORT_NAME");
    }
}

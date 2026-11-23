mod support;

use std::process::Command;
use support::data;
use support::run::run_cli;
use support::sandbox::Sandbox;

#[test]
fn preserves_input_fidelity_and_reports_not_found_accurately() {
    let sb = Sandbox::new("r16_not_found");
    let dd = sb.data_dir();
    let cwd = sb.work_dir();

    let missing = dd.join("does_not_exist_12345.txt");
    let existing = data::plain_file(&dd, "exists.txt", "hi");
    let a1 = sb.checked_arg(&existing);
    let a2 = sb.checked_arg(&missing);

    let res = run_cli(&cwd, &["put", "--format", "json", &a1, &a2]);
    assert_eq!(res.exit_code, 2, "stdout={}", res.stdout);
    let j = res.json();
    let items = j["items"].as_array().unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(j["summary"]["total"], 2);
    assert_eq!(items[0]["input"].as_str().unwrap(), a1);
    assert_eq!(items[1]["input"].as_str().unwrap(), a2);
    assert_eq!(items[1]["outcome"], "blocked");
    assert_eq!(items[1]["code"], "E_NOT_FOUND");
}

#[test]
fn access_denied_is_not_misreported_as_not_found() {
    let sb = Sandbox::new("r16_access_denied");
    let dd = sb.data_dir();
    let cwd = sb.work_dir();

    let restricted_dir = data::empty_dir(&dd, "restricted");
    let secret = data::plain_file(&restricted_dir, "secret.txt", "denied");
    let secret_arg = sb.checked_arg(&secret);

    let user = std::env::var("USERNAME").unwrap_or_default();
    if user.is_empty() {
        return;
    }

    let dir_str = restricted_dir.to_string_lossy().to_string();
    let deny_spec = format!("{}:(OI)(CI)(F)", user);
    // Deny access on restricted_dir so probing secret.txt yields ERROR_ACCESS_DENIED (5)
    let deny = Command::new("icacls")
        .args([dir_str.as_str(), "/deny", deny_spec.as_str()])
        .output();

    let ok = deny.as_ref().map(|o| o.status.success()).unwrap_or(false);
    if !ok {
        return;
    }

    let res = run_cli(&cwd, &["put", "--format", "json", &secret_arg]);

    // Always restore permissions before assertions so sandbox Drop can clean up cleanly
    let _ = Command::new("icacls")
        .args([dir_str.as_str(), "/remove:d", user.as_str()])
        .output();

    assert_ne!(res.exit_code, 0, "denied target must not succeed");
    let j = res.json();
    let code = j["items"][0]["code"].as_str().unwrap_or("");
    assert_ne!(
        code, "E_NOT_FOUND",
        "R16 violation: permission error misreported as E_NOT_FOUND! stdout={}",
        res.stdout
    );
    assert_eq!(
        code, "E_ACCESS_DENIED",
        "expected E_ACCESS_DENIED, got {} stdout={}",
        code, res.stdout
    );
}

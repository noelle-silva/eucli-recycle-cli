#[path = "../base/support/mod.rs"]
mod support;

use std::collections::HashSet;
use std::time::Duration;
use support::data;
use support::run::{run_cli, run_cli_with_timeout};
use support::sandbox::Sandbox;

const BIN_SCAN_TIMEOUT: Duration = Duration::from_secs(120);

#[test]
fn list_from_boundary_name_case_time_and_pagination_work_together() {
    let sb = Sandbox::new("s05_list_filters");
    let cwd = sb.work_dir();
    let dir_a = data::empty_dir(sb.path(), "proj");
    let dir_a_sub = data::empty_dir(&dir_a, "nested");
    let dir_b = data::empty_dir(sb.path(), "proj_extra");

    let f1 = data::plain_file(&dir_a, "alpha_Case.txt", "1");
    let f2 = data::plain_file(&dir_a, "beta_case.txt", "2");
    let f3 = data::plain_file(&dir_a, "gamma_other.log", "3");
    let f4 = data::plain_file(&dir_a_sub, "delta_Case.txt", "4");
    let f5 = data::plain_file(&dir_b, "outside_Case.txt", "5");

    let args_owned: Vec<String> = [&f1, &f2, &f3, &f4, &f5]
        .iter()
        .map(|p| sb.checked_arg(p))
        .collect();
    let mut put_args = vec!["put", "--format", "json"];
    for s in &args_owned {
        put_args.push(s.as_str());
    }
    let put = run_cli(&cwd, &put_args);
    assert_eq!(put.exit_code, 0, "stdout={}", put.stdout);

    let dir_a_s = dir_a.to_string_lossy().to_string();

    // 1. --from boundary: matches proj + proj\nested (4 items), must NOT match proj_extra
    let l_from = run_cli_with_timeout(
        &cwd,
        &["list", "--from", &dir_a_s, "--format", "json"],
        BIN_SCAN_TIMEOUT,
    );
    assert_eq!(l_from.exit_code, 0, "stdout={}", l_from.stdout);
    let j_from = l_from.json();
    assert_eq!(j_from["summary"]["total"], 4, "proj_extra must not leak into --from proj");
    assert_eq!(j_from["summary"]["returned"], 4);

    // 2. --name is case-sensitive and literal (no wildcards)
    let l_case = run_cli_with_timeout(
        &cwd,
        &["list", "--from", &dir_a_s, "--name", "Case", "--format", "json"],
        BIN_SCAN_TIMEOUT,
    );
    assert_eq!(l_case.exit_code, 0);
    let case_items = l_case.json()["items"].as_array().unwrap().clone();
    assert_eq!(case_items.len(), 2, "only alpha_Case.txt and delta_Case.txt match `Case`");

    let l_wild = run_cli_with_timeout(
        &cwd,
        &["list", "--from", &dir_a_s, "--name", "*.txt", "--format", "json"],
        BIN_SCAN_TIMEOUT,
    );
    assert_eq!(l_wild.exit_code, 0);
    assert_eq!(
        l_wild.json()["items"].as_array().unwrap().len(),
        0,
        "wildcards must not expand in --name"
    );

    // 3. --after / --before time filtering
    let l_future = run_cli_with_timeout(
        &cwd,
        &["list", "--from", &dir_a_s, "--after", "2099-01-01", "--format", "json"],
        BIN_SCAN_TIMEOUT,
    );
    assert_eq!(l_future.exit_code, 0);
    assert_eq!(l_future.json()["summary"]["total"], 0);

    let l_past = run_cli_with_timeout(
        &cwd,
        &["list", "--from", &dir_a_s, "--before", "2000-01-01T00:00:00", "--format", "json"],
        BIN_SCAN_TIMEOUT,
    );
    assert_eq!(l_past.exit_code, 0);
    assert_eq!(l_past.json()["summary"]["total"], 0);

    let l_range = run_cli_with_timeout(
        &cwd,
        &[
            "list",
            "--from",
            &dir_a_s,
            "--after",
            "2020-01-01",
            "--before",
            "2099-12-31",
            "--format",
            "json",
        ],
        BIN_SCAN_TIMEOUT,
    );
    assert_eq!(l_range.exit_code, 0);
    assert_eq!(l_range.json()["summary"]["total"], 4);

    // 4. Pagination: --limit 2 --offset 0 and --offset 2 cover all 4 items with no overlap
    let page1 = run_cli_with_timeout(
        &cwd,
        &["list", "--from", &dir_a_s, "--limit", "2", "--offset", "0", "--format", "json"],
        BIN_SCAN_TIMEOUT,
    );
    let page2 = run_cli_with_timeout(
        &cwd,
        &["list", "--from", &dir_a_s, "--limit", "2", "--offset", "2", "--format", "json"],
        BIN_SCAN_TIMEOUT,
    );
    assert_eq!(page1.exit_code, 0);
    assert_eq!(page2.exit_code, 0);
    let p1j = page1.json();
    let p2j = page2.json();
    assert_eq!(p1j["summary"]["total"], 4);
    assert_eq!(p1j["summary"]["returned"], 2);
    assert_eq!(p2j["summary"]["total"], 4);
    assert_eq!(p2j["summary"]["returned"], 2);

    let mut seen: HashSet<String> = HashSet::new();
    for it in p1j["items"].as_array().unwrap().iter().chain(p2j["items"].as_array().unwrap()) {
        seen.insert(it["trash_id"].as_str().unwrap().to_string());
    }
    assert_eq!(seen.len(), 4, "two pages must be disjoint and complete");
}

#[test]
fn list_rejects_invalid_filter_arguments_with_json_error() {
    let sb = Sandbox::new("s05_list_bad_args");
    let cwd = sb.work_dir();

    let bad_from = run_cli(&cwd, &["list", "--from", r"relative\dir", "--format", "json"]);
    assert_eq!(bad_from.exit_code, 1);
    assert_eq!(bad_from.json()["error"]["code"], "E_NOT_ABSOLUTE");

    let bad_date = run_cli(&cwd, &["list", "--after", "not-a-date", "--format", "json"]);
    assert_eq!(bad_date.exit_code, 1);
    assert_eq!(bad_date.json()["error"]["code"], "E_UNKNOWN_ARG");

    let bad_limit = run_cli(&cwd, &["list", "--limit", "abc", "--format", "json"]);
    assert_eq!(bad_limit.exit_code, 1);
    assert_eq!(bad_limit.json()["error"]["code"], "E_UNKNOWN_ARG");
}

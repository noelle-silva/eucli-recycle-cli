mod support;

use support::data;
use support::run::{parse_human_items, run_cli};
use support::sandbox::Sandbox;

#[test]
fn human_and_json_outputs_match_for_dry_run_and_blocked_batches() {
    let sb = Sandbox::new("r17_single_source");
    let dd = sb.data_dir();
    let cwd = sb.work_dir();

    let f1 = data::plain_file(&dd, "one.txt", "1");
    let d2 = data::empty_dir(&dd, "two_dir");
    let missing = dd.join("missing.txt");
    let a1 = sb.checked_arg(&f1);
    let a2 = sb.checked_arg(&d2);
    let a3 = sb.checked_arg(&missing);

    // Blocked batch in both formats
    let r_json = run_cli(&cwd, &["put", "--format", "json", &a1, &a2, &a3]);
    let r_human = run_cli(&cwd, &["put", "--format", "human", &a1, &a2, &a3]);

    assert_eq!(r_json.exit_code, r_human.exit_code);
    assert_eq!(r_json.exit_code, 2);

    let j = r_json.json();
    let j_items = j["items"].as_array().unwrap();
    let h_items = parse_human_items(&r_human.stdout);
    assert_eq!(
        j_items.len(),
        h_items.len(),
        "item count mismatch:\nhuman:\n{}\njson:\n{}",
        r_human.stdout,
        r_json.stdout
    );
    for (ji, hi) in j_items.iter().zip(h_items.iter()) {
        assert_eq!(ji["outcome"].as_str().unwrap(), hi.outcome);
        if hi.outcome == "blocked" || hi.outcome == "failed" {
            assert_eq!(ji["code"].as_str().unwrap(), hi.code.as_deref().unwrap_or(""));
        }
    }

    // Dry-run planned batch in both formats
    let r_plan_json = run_cli(&cwd, &["put", "--dry-run", "--format", "json", &a1]);
    let r_plan_human = run_cli(&cwd, &["put", "--dry-run", "--format", "human", &a1]);
    assert_eq!(r_plan_json.exit_code, 0);
    assert_eq!(r_plan_human.exit_code, 0);
    let pj = r_plan_json.json();
    let ph = parse_human_items(&r_plan_human.stdout);
    assert_eq!(pj["items"].as_array().unwrap().len(), ph.len());
    assert_eq!(pj["items"][0]["outcome"], ph[0].outcome);
}

#[test]
fn human_and_json_outputs_match_for_successful_put() {
    let sb = Sandbox::new("r17_success_pair");
    let dd = sb.data_dir();
    let cwd = sb.work_dir();

    let fa = data::plain_file(&dd, "pair_a.txt", "a");
    let fb = data::plain_file(&dd, "pair_b.txt", "b");
    let aa = sb.checked_arg(&fa);
    let ab = sb.checked_arg(&fb);

    let r_json = run_cli(&cwd, &["put", "--format", "json", &aa]);
    let r_human = run_cli(&cwd, &["put", "--format", "human", &ab]);
    assert_eq!(r_json.exit_code, 0);
    assert_eq!(r_human.exit_code, 0);
    let h_items = parse_human_items(&r_human.stdout);
    assert_eq!(h_items.len(), 1);
    assert_eq!(h_items[0].outcome, "succeeded");
    assert_eq!(r_json.json()["items"][0]["outcome"], "succeeded");
    assert!(r_human.stdout.contains("退出码：0"));
}

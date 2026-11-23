mod support;

use eucli_recycle::model::error_code::ErrorCode;
use eucli_recycle::model::outcome::{ItemOutcome, VerificationInfo};
use eucli_recycle::platform::{PlatformError, RecycleBin, RecycleItemOutcome, ResolvedTarget, TrashEntry, ListFilter};
use support::data;
use support::run::run_cli;
use support::sandbox::Sandbox;

#[test]
fn successful_put_returns_trash_id_verifiable_in_list() {
    let sb = Sandbox::new("r15_verify_ok");
    let dd = sb.data_dir();
    let cwd = sb.work_dir();

    let f = data::plain_file(&dd, "r15_verifiable_unique.txt", "verify me");
    let arg = sb.checked_arg(&f);

    let put_res = run_cli(&cwd, &["put", "--format", "json", &arg]);
    assert_eq!(put_res.exit_code, 0, "stdout={}", put_res.stdout);
    let put_json = put_res.json();
    assert_eq!(put_json["items"][0]["outcome"], "succeeded");
    let trash_id = put_json["items"][0]["trash_id"]
        .as_str()
        .expect("trash_id must be present")
        .to_string();
    assert_eq!(trash_id.len(), 16, "8 bytes hex = 16 chars");

    let dd_str = dd.to_string_lossy().to_string();
    let list_res = run_cli(
        &cwd,
        &[
            "list",
            "--from",
            &dd_str,
            "--name",
            "r15_verifiable_unique.txt",
            "--format",
            "json",
        ],
    );
    assert_eq!(list_res.exit_code, 0, "stdout={}", list_res.stdout);
    let list_json = list_res.json();
    let items = list_json["items"].as_array().unwrap();
    assert!(
        items.iter().any(|it| it["trash_id"].as_str() == Some(trash_id.as_str())),
        "trash_id `{}` from put must be found in list: {}",
        trash_id,
        list_res.stdout
    );
}

struct UnverifiableMockBin;

impl RecycleBin for UnverifiableMockBin {
    fn recycle(&self, targets: &[ResolvedTarget]) -> Vec<RecycleItemOutcome> {
        targets
            .iter()
            .map(|_| RecycleItemOutcome {
                outcome: ItemOutcome::Unknown,
                code: ErrorCode::EVerifyFailed,
                message: "核实失败：无法确认条目已进入回收站".to_string(),
                trash_id: None,
                verified: Some(VerificationInfo {
                    absent_at_source: Some(true),
                    present_in_bin: Some(false),
                    present_at_original: None,
                    absent_in_bin: None,
                }),
            })
            .collect()
    }

    fn enumerate(&self, _filter: &ListFilter) -> Result<Vec<TrashEntry>, PlatformError> {
        Ok(vec![])
    }

    fn restore(&self, _entries: &[TrashEntry]) -> Vec<RecycleItemOutcome> {
        vec![]
    }
}

#[test]
fn failed_verification_must_report_unknown_and_exit_4_never_succeeded() {
    let sb = Sandbox::new("r15_verify_fail");
    let dd = sb.data_dir();
    let cwd = sb.work_dir();
    let f = data::plain_file(&dd, "ghost.txt", "data");
    let arg = sb.checked_arg(&f);

    let report = eucli_recycle::run_put_with_bin(
        &cwd,
        &[arg],
        false,
        false,
        None,
        &UnverifiableMockBin,
    );

    assert_eq!(report.exit_code, 4, "unknown outcome must yield exit code 4");
    assert_eq!(report.summary.succeeded, 0);
    assert_eq!(report.summary.unknown, 1);
    assert_eq!(report.items[0].outcome.as_str(), "unknown");
    assert_eq!(report.items[0].code.as_str(), "E_VERIFY_FAILED");
}

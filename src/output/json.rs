use crate::model::report::{ActionReport, ListReport};

/// Section 9.3 Machine-readable JSON output rendered from the same internal report (R17).
pub fn render_action_json(report: &ActionReport) -> String {
    let mut s = serde_json::to_string_pretty(report).expect("serialize ActionReport");
    s.push('\n');
    s
}

pub fn render_list_json(report: &ListReport) -> String {
    let mut s = serde_json::to_string_pretty(report).expect("serialize ListReport");
    s.push('\n');
    s
}

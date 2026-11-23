pub mod human;
pub mod json;

use crate::cli::parse::OutputFormat;
use crate::model::report::{ActionReport, ListReport};

pub fn render_action(report: &ActionReport, format: OutputFormat) -> String {
    match format {
        OutputFormat::Human => human::render_action_human(report),
        OutputFormat::Json => json::render_action_json(report),
    }
}

pub fn render_list(report: &ListReport, format: OutputFormat) -> String {
    match format {
        OutputFormat::Human => human::render_list_human(report),
        OutputFormat::Json => json::render_list_json(report),
    }
}

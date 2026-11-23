pub mod help;
pub mod parse;

use crate::cli::parse::{HelpTopic, ParsedCommand};
use crate::model::report::ActionReport;
use crate::output;
use std::io::{self, Write};

fn write_utf8_stdout(text: &str) {
    let mut out = io::stdout().lock();
    let _ = out.write_all(text.as_bytes());
    let _ = out.flush();
}

pub fn run_from_env() -> i32 {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let started_at = ActionReport::now_iso8601();

    let parsed = match parse::parse_args(&args) {
        Ok(cmd) => cmd,
        Err(err) => {
            let report = ActionReport::from_top_error(
                &err.command,
                started_at,
                err.code,
                err.message,
                1,
            );
            let rendered = output::render_action(&report, err.format);
            write_utf8_stdout(&rendered);
            return report.exit_code;
        }
    };

    match parsed {
        ParsedCommand::Version => {
            write_utf8_stdout(&format!(
                "eucli-recycle {}\n",
                env!("CARGO_PKG_VERSION")
            ));
            0
        }
        ParsedCommand::Help(topic) => {
            let text = match topic {
                HelpTopic::Main => help::main_help(),
                HelpTopic::Put => help::put_help(),
                HelpTopic::List => help::list_help(),
                HelpTopic::Restore => help::restore_help(),
            };
            write_utf8_stdout(&text);
            0
        }
        ParsedCommand::Put(put_args) => {
            let cwd = match std::env::current_dir() {
                Ok(p) => p,
                Err(e) => {
                    let report = ActionReport::from_top_error(
                        "put",
                        started_at,
                        crate::model::error_code::ErrorCode::EUnknownSystem,
                        format!("无法获取当前工作目录：{}", e),
                        1,
                    );
                    write_utf8_stdout(&output::render_action(&report, put_args.format));
                    return report.exit_code;
                }
            };
            let report = crate::execute_put(&cwd, &put_args, None);
            write_utf8_stdout(&output::render_action(&report, put_args.format));
            report.exit_code
        }
        ParsedCommand::List(list_args) => {
            let report = crate::execute_list(&list_args);
            write_utf8_stdout(&output::render_list(&report, list_args.format));
            report.exit_code
        }
        ParsedCommand::Restore(restore_args) => {
            let cwd = std::env::current_dir().unwrap_or_default();
            let report = crate::execute_restore(&cwd, &restore_args);
            write_utf8_stdout(&output::render_action(&report, restore_args.format));
            report.exit_code
        }
    }
}

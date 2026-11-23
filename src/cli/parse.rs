use crate::model::error_code::ErrorCode;
use crate::rules::absolute::validate_and_normalize_path;
use chrono::{DateTime, Local, NaiveDate, NaiveDateTime, TimeZone};
use std::path::PathBuf;

pub const MAX_TARGETS: usize = 1000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Human,
    Json,
}

#[derive(Debug, Clone)]
pub enum ParsedCommand {
    Version,
    Help(HelpTopic),
    Put(PutArgs),
    List(ListArgs),
    Restore(RestoreArgs),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HelpTopic {
    Main,
    Put,
    List,
    Restore,
}

#[derive(Debug, Clone)]
pub struct PutArgs {
    pub dir: bool,
    pub dry_run: bool,
    pub format: OutputFormat,
    pub config: Option<PathBuf>,
    pub targets: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ListArgs {
    pub from: Option<String>,
    pub name: Option<String>,
    pub after: Option<DateTime<Local>>,
    pub before: Option<DateTime<Local>>,
    pub limit: Option<usize>,
    pub offset: usize,
    pub format: OutputFormat,
}

#[derive(Debug, Clone)]
pub struct RestoreArgs {
    pub format: OutputFormat,
    pub config: Option<PathBuf>,
    pub trash_ids: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ParseError {
    pub command: String,
    pub format: OutputFormat,
    pub code: ErrorCode,
    pub message: String,
}

/// Pre-scans args for `--format json` so parse errors still honor JSON output format (R17 / 9.3).
fn detect_format_hint(args: &[String]) -> OutputFormat {
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--format" {
            if let Some(val) = args.get(i + 1) {
                if val == "json" {
                    return OutputFormat::Json;
                }
            }
            i += 2;
            continue;
        }
        if let Some(val) = args[i].strip_prefix("--format=") {
            if val == "json" {
                return OutputFormat::Json;
            }
        }
        i += 1;
    }
    OutputFormat::Human
}

pub fn parse_args(args: &[String]) -> Result<ParsedCommand, ParseError> {
    let format_hint = detect_format_hint(args);

    if args.is_empty() {
        return Err(ParseError {
            command: "eucli-recycle".to_string(),
            format: format_hint,
            code: ErrorCode::ENoArgs,
            message: "未指定子命令；可用子命令：put、list、restore、help".to_string(),
        });
    }

    match args[0].as_str() {
        "--version" => {
            if args.len() > 1 {
                return Err(ParseError {
                    command: "eucli-recycle".to_string(),
                    format: format_hint,
                    code: ErrorCode::EUnknownArg,
                    message: format!("不认识的参数：{}", args[1]),
                });
            }
            Ok(ParsedCommand::Version)
        }
        "--help" => {
            if args.len() > 1 {
                return Err(ParseError {
                    command: "eucli-recycle".to_string(),
                    format: format_hint,
                    code: ErrorCode::EUnknownArg,
                    message: format!("不认识的参数：{}", args[1]),
                });
            }
            Ok(ParsedCommand::Help(HelpTopic::Main))
        }
        "help" => {
            if args.len() == 1 {
                Ok(ParsedCommand::Help(HelpTopic::Main))
            } else if args.len() == 2 {
                match args[1].as_str() {
                    "put" => Ok(ParsedCommand::Help(HelpTopic::Put)),
                    "list" => Ok(ParsedCommand::Help(HelpTopic::List)),
                    "restore" => Ok(ParsedCommand::Help(HelpTopic::Restore)),
                    other => Err(ParseError {
                        command: "help".to_string(),
                        format: format_hint,
                        code: ErrorCode::EUnknownArg,
                        message: format!("不认识的子命令：{}", other),
                    }),
                }
            } else {
                Err(ParseError {
                    command: "help".to_string(),
                    format: format_hint,
                    code: ErrorCode::EUnknownArg,
                    message: format!("多余的参数：{}", args[2]),
                })
            }
        }
        "put" => parse_put(&args[1..], format_hint),
        "list" => parse_list(&args[1..], format_hint),
        "restore" => parse_restore(&args[1..], format_hint),
        "--format" => {
            // Only `--format <fmt>` with no subcommand -> E_NO_ARGS
            if args.len() <= 2 {
                Err(ParseError {
                    command: "eucli-recycle".to_string(),
                    format: format_hint,
                    code: ErrorCode::ENoArgs,
                    message: "未指定子命令；可用子命令：put、list、restore、help".to_string(),
                })
            } else {
                Err(ParseError {
                    command: "eucli-recycle".to_string(),
                    format: format_hint,
                    code: ErrorCode::EUnknownArg,
                    message: format!("不认识的参数或子命令：{}", args[0]),
                })
            }
        }
        other => Err(ParseError {
            command: "eucli-recycle".to_string(),
            format: format_hint,
            code: ErrorCode::EUnknownArg,
            message: format!("不认识的子命令或参数：{}（仅支持 put、list、restore、help）", other),
        }),
    }
}

fn parse_format_value(cmd: &str, val: &str, hint: OutputFormat) -> Result<OutputFormat, ParseError> {
    match val {
        "human" => Ok(OutputFormat::Human),
        "json" => Ok(OutputFormat::Json),
        _ => Err(ParseError {
            command: cmd.to_string(),
            format: hint,
            code: ErrorCode::EUnknownArg,
            message: format!("--format 取值非法：{}（仅支持 human 或 json）", val),
        }),
    }
}

fn parse_put(args: &[String], hint: OutputFormat) -> Result<ParsedCommand, ParseError> {
    let mut dir = false;
    let mut dry_run = false;
    let mut format = OutputFormat::Human;
    let mut config: Option<PathBuf> = None;
    let mut targets: Vec<String> = Vec::new();

    let mut i = 0;
    while i < args.len() {
        let a = args[i].as_str();
        if a == "--help" {
            return Ok(ParsedCommand::Help(HelpTopic::Put));
        } else if a == "--dir" {
            dir = true;
            i += 1;
        } else if a == "--dry-run" {
            dry_run = true;
            i += 1;
        } else if a == "--format" {
            let val = args.get(i + 1).ok_or_else(|| ParseError {
                command: "put".to_string(),
                format: hint,
                code: ErrorCode::EUnknownArg,
                message: "--format 缺少取值（human 或 json）".to_string(),
            })?;
            format = parse_format_value("put", val, hint)?;
            i += 2;
        } else if let Some(val) = a.strip_prefix("--format=") {
            format = parse_format_value("put", val, hint)?;
            i += 1;
        } else if a == "--config" {
            let val = args.get(i + 1).ok_or_else(|| ParseError {
                command: "put".to_string(),
                format: hint,
                code: ErrorCode::EUnknownArg,
                message: "--config 缺少路径参数".to_string(),
            })?;
            config = Some(PathBuf::from(val));
            i += 2;
        } else if let Some(val) = a.strip_prefix("--config=") {
            config = Some(PathBuf::from(val));
            i += 1;
        } else if a.starts_with('-') {
            return Err(ParseError {
                command: "put".to_string(),
                format: hint,
                code: ErrorCode::EUnknownArg,
                message: format!("不认识的参数：{}（本工具不兼容 rm 参数风格）", a),
            });
        } else {
            targets.push(a.to_string());
            i += 1;
        }
    }

    if targets.is_empty() {
        return Err(ParseError {
            command: "put".to_string(),
            format,
            code: ErrorCode::ENoArgs,
            message: "put 缺少目标路径，请至少提供一个绝对路径".to_string(),
        });
    }

    if targets.len() > MAX_TARGETS {
        return Err(ParseError {
            command: "put".to_string(),
            format,
            code: ErrorCode::ETooManyTargets,
            message: format!("目标数量为 {}，超过单次上限 {}", targets.len(), MAX_TARGETS),
        });
    }

    Ok(ParsedCommand::Put(PutArgs {
        dir,
        dry_run,
        format,
        config,
        targets,
    }))
}

fn parse_list(args: &[String], hint: OutputFormat) -> Result<ParsedCommand, ParseError> {
    let mut from: Option<String> = None;
    let mut name: Option<String> = None;
    let mut after: Option<DateTime<Local>> = None;
    let mut before: Option<DateTime<Local>> = None;
    let mut limit: Option<usize> = None;
    let mut offset: usize = 0;
    let mut format = OutputFormat::Human;

    let mut i = 0;
    while i < args.len() {
        let a = args[i].as_str();
        if a == "--help" {
            return Ok(ParsedCommand::Help(HelpTopic::List));
        } else if a == "--format" {
            let val = args.get(i + 1).ok_or_else(|| ParseError {
                command: "list".to_string(),
                format: hint,
                code: ErrorCode::EUnknownArg,
                message: "--format 缺少取值".to_string(),
            })?;
            format = parse_format_value("list", val, hint)?;
            i += 2;
        } else if let Some(val) = a.strip_prefix("--format=") {
            format = parse_format_value("list", val, hint)?;
            i += 1;
        } else if a == "--from" {
            let val = args.get(i + 1).ok_or_else(|| ParseError {
                command: "list".to_string(),
                format: hint,
                code: ErrorCode::EUnknownArg,
                message: "--from 缺少绝对路径参数".to_string(),
            })?;
            let norm = validate_and_normalize_path(val).map_err(|(code, msg)| ParseError {
                command: "list".to_string(),
                format: hint,
                code,
                message: format!("--from 路径无效：{}", msg),
            })?;
            from = Some(norm.normalized);
            i += 2;
        } else if let Some(val) = a.strip_prefix("--from=") {
            let norm = validate_and_normalize_path(val).map_err(|(code, msg)| ParseError {
                command: "list".to_string(),
                format: hint,
                code,
                message: format!("--from 路径无效：{}", msg),
            })?;
            from = Some(norm.normalized);
            i += 1;
        } else if a == "--name" {
            let val = args.get(i + 1).ok_or_else(|| ParseError {
                command: "list".to_string(),
                format: hint,
                code: ErrorCode::EUnknownArg,
                message: "--name 缺少子串参数".to_string(),
            })?;
            name = Some(val.clone());
            i += 2;
        } else if let Some(val) = a.strip_prefix("--name=") {
            name = Some(val.to_string());
            i += 1;
        } else if a == "--after" {
            let val = args.get(i + 1).ok_or_else(|| ParseError {
                command: "list".to_string(),
                format: hint,
                code: ErrorCode::EUnknownArg,
                message: "--after 缺少时间参数".to_string(),
            })?;
            after = Some(parse_time_bound(val, false, hint)?);
            i += 2;
        } else if let Some(val) = a.strip_prefix("--after=") {
            after = Some(parse_time_bound(val, false, hint)?);
            i += 1;
        } else if a == "--before" {
            let val = args.get(i + 1).ok_or_else(|| ParseError {
                command: "list".to_string(),
                format: hint,
                code: ErrorCode::EUnknownArg,
                message: "--before 缺少时间参数".to_string(),
            })?;
            before = Some(parse_time_bound(val, true, hint)?);
            i += 2;
        } else if let Some(val) = a.strip_prefix("--before=") {
            before = Some(parse_time_bound(val, true, hint)?);
            i += 1;
        } else if a == "--limit" {
            let val = args.get(i + 1).ok_or_else(|| ParseError {
                command: "list".to_string(),
                format: hint,
                code: ErrorCode::EUnknownArg,
                message: "--limit 缺少数字参数".to_string(),
            })?;
            limit = Some(val.parse::<usize>().map_err(|_| ParseError {
                command: "list".to_string(),
                format: hint,
                code: ErrorCode::EUnknownArg,
                message: format!("--limit 需要非负整数：{}", val),
            })?);
            i += 2;
        } else if let Some(val) = a.strip_prefix("--limit=") {
            limit = Some(val.parse::<usize>().map_err(|_| ParseError {
                command: "list".to_string(),
                format: hint,
                code: ErrorCode::EUnknownArg,
                message: format!("--limit 需要非负整数：{}", val),
            })?);
            i += 1;
        } else if a == "--offset" {
            let val = args.get(i + 1).ok_or_else(|| ParseError {
                command: "list".to_string(),
                format: hint,
                code: ErrorCode::EUnknownArg,
                message: "--offset 缺少数字参数".to_string(),
            })?;
            offset = val.parse::<usize>().map_err(|_| ParseError {
                command: "list".to_string(),
                format: hint,
                code: ErrorCode::EUnknownArg,
                message: format!("--offset 需要非负整数：{}", val),
            })?;
            i += 2;
        } else if let Some(val) = a.strip_prefix("--offset=") {
            offset = val.parse::<usize>().map_err(|_| ParseError {
                command: "list".to_string(),
                format: hint,
                code: ErrorCode::EUnknownArg,
                message: format!("--offset 需要非负整数：{}", val),
            })?;
            i += 1;
        } else {
            return Err(ParseError {
                command: "list".to_string(),
                format: hint,
                code: ErrorCode::EUnknownArg,
                message: format!("不认识的参数：{}", a),
            });
        }
    }

    Ok(ParsedCommand::List(ListArgs {
        from,
        name,
        after,
        before,
        limit,
        offset,
        format,
    }))
}

fn parse_time_bound(val: &str, end_of_day: bool, hint: OutputFormat) -> Result<DateTime<Local>, ParseError> {
    if let Ok(dt) = NaiveDateTime::parse_from_str(val, "%Y-%m-%dT%H:%M:%S") {
        return Local
            .from_local_datetime(&dt)
            .earliest()
            .ok_or_else(|| ParseError {
                command: "list".to_string(),
                format: hint,
                code: ErrorCode::EUnknownArg,
                message: format!("无法解析本地时间：{}", val),
            });
    }
    if let Ok(d) = NaiveDate::parse_from_str(val, "%Y-%m-%d") {
        let dt = if end_of_day {
            d.and_hms_opt(23, 59, 59).unwrap()
        } else {
            d.and_hms_opt(0, 0, 0).unwrap()
        };
        return Local
            .from_local_datetime(&dt)
            .earliest()
            .ok_or_else(|| ParseError {
                command: "list".to_string(),
                format: hint,
                code: ErrorCode::EUnknownArg,
                message: format!("无法解析本地日期：{}", val),
            });
    }
    Err(ParseError {
        command: "list".to_string(),
        format: hint,
        code: ErrorCode::EUnknownArg,
        message: format!(
            "时间格式非法：{}（应为 YYYY-MM-DD 或 YYYY-MM-DDTHH:MM:SS）",
            val
        ),
    })
}

fn parse_restore(args: &[String], hint: OutputFormat) -> Result<ParsedCommand, ParseError> {
    let mut format = OutputFormat::Human;
    let mut config: Option<PathBuf> = None;
    let mut trash_ids: Vec<String> = Vec::new();

    let mut i = 0;
    while i < args.len() {
        let a = args[i].as_str();
        if a == "--help" {
            return Ok(ParsedCommand::Help(HelpTopic::Restore));
        } else if a == "--format" {
            let val = args.get(i + 1).ok_or_else(|| ParseError {
                command: "restore".to_string(),
                format: hint,
                code: ErrorCode::EUnknownArg,
                message: "--format 缺少取值".to_string(),
            })?;
            format = parse_format_value("restore", val, hint)?;
            i += 2;
        } else if let Some(val) = a.strip_prefix("--format=") {
            format = parse_format_value("restore", val, hint)?;
            i += 1;
        } else if a == "--config" {
            let val = args.get(i + 1).ok_or_else(|| ParseError {
                command: "restore".to_string(),
                format: hint,
                code: ErrorCode::EUnknownArg,
                message: "--config 缺少路径参数".to_string(),
            })?;
            config = Some(PathBuf::from(val));
            i += 2;
        } else if let Some(val) = a.strip_prefix("--config=") {
            config = Some(PathBuf::from(val));
            i += 1;
        } else if a.starts_with('-') {
            return Err(ParseError {
                command: "restore".to_string(),
                format: hint,
                code: ErrorCode::EUnknownArg,
                message: format!("不认识的参数：{}", a),
            });
        } else {
            trash_ids.push(a.to_string());
            i += 1;
        }
    }

    if trash_ids.is_empty() {
        return Err(ParseError {
            command: "restore".to_string(),
            format,
            code: ErrorCode::ENoArgs,
            message: "restore 缺少条目标识（请先运行 list 获取 trash_id）".to_string(),
        });
    }

    if trash_ids.len() > MAX_TARGETS {
        return Err(ParseError {
            command: "restore".to_string(),
            format,
            code: ErrorCode::ETooManyTargets,
            message: format!("条目标识数量为 {}，超过单次上限 {}", trash_ids.len(), MAX_TARGETS),
        });
    }

    Ok(ParsedCommand::Restore(RestoreArgs {
        format,
        config,
        trash_ids,
    }))
}

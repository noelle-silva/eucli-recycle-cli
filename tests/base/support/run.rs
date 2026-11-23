#![allow(dead_code)]

use serde_json::Value;
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone)]
pub struct RunResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub elapsed: Duration,
}

impl RunResult {
    pub fn json(&self) -> Value {
        serde_json::from_str(&self.stdout).unwrap_or_else(|e| {
            panic!(
                "stdout is not valid JSON: {}\nexit_code={}\nstdout:\n{}\nstderr:\n{}",
                e, self.exit_code, self.stdout, self.stderr
            )
        })
    }
}

pub fn bin_path() -> &'static str {
    env!("CARGO_BIN_EXE_eucli-recycle")
}

pub fn run_cli(cwd: &Path, args: &[&str]) -> RunResult {
    run_cli_with_options(cwd, args, DEFAULT_TIMEOUT, &[], false)
}

pub fn run_cli_with_timeout(cwd: &Path, args: &[&str], timeout: Duration) -> RunResult {
    run_cli_with_options(cwd, args, timeout, &[], false)
}

pub fn run_cli_with_options(
    cwd: &Path,
    args: &[&str],
    timeout: Duration,
    extra_env: &[(&str, &str)],
    clear_env: bool,
) -> RunResult {
    let mut cmd = Command::new(bin_path());
    cmd.current_dir(cwd)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    if clear_env {
        cmd.env_clear();
        // Keep SystemRoot so Windows COM/DLL loader works cleanly.
        if let Ok(sr) = std::env::var("SystemRoot") {
            cmd.env("SystemRoot", sr);
        }
    }
    for (k, v) in extra_env {
        cmd.env(k, v);
    }

    let start = Instant::now();
    let mut child = cmd.spawn().expect("spawn eucli-recycle binary");

    let mut stdout_pipe = child.stdout.take().expect("stdout pipe");
    let mut stderr_pipe = child.stderr.take().expect("stderr pipe");

    let t_out = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = stdout_pipe.read_to_end(&mut buf);
        buf
    });
    let t_err = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = stderr_pipe.read_to_end(&mut buf);
        buf
    });

    let status = loop {
        match child.try_wait().expect("try_wait") {
            Some(st) => break st,
            None => {
                if start.elapsed() >= timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    panic!(
                        "TIMEOUT after {:?} running {:?} — 疑似出现交互或挂起 (R4)",
                        timeout, args
                    );
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        }
    };

    let stdout_bytes = t_out.join().expect("join stdout thread");
    let stderr_bytes = t_err.join().expect("join stderr thread");

    RunResult {
        exit_code: status.code().unwrap_or(-1),
        stdout: String::from_utf8(stdout_bytes).expect("stdout must be valid UTF-8 (R23)"),
        stderr: String::from_utf8_lossy(&stderr_bytes).to_string(),
        elapsed: start.elapsed(),
    }
}

/// Runs the CLI with an open piped stdin populated with `stdin_data` (or kept open briefly)
/// to prove the tool never reads stdin and never hangs in pipelines (R4 / R23).
pub fn run_cli_with_piped_stdin(
    cwd: &Path,
    args: &[&str],
    stdin_data: &[u8],
    timeout: Duration,
) -> RunResult {
    let mut cmd = Command::new(bin_path());
    cmd.current_dir(cwd)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let start = Instant::now();
    let mut child = cmd.spawn().expect("spawn eucli-recycle binary");

    if let Some(mut stdin_pipe) = child.stdin.take() {
        let data = stdin_data.to_vec();
        std::thread::spawn(move || {
            let _ = stdin_pipe.write_all(&data);
            let _ = stdin_pipe.flush();
            // Intentionally hold pipe open briefly before closing to ensure CLI doesn't wait for EOF
            std::thread::sleep(Duration::from_millis(300));
            drop(stdin_pipe);
        });
    }

    let mut stdout_pipe = child.stdout.take().expect("stdout pipe");
    let mut stderr_pipe = child.stderr.take().expect("stderr pipe");

    let t_out = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = stdout_pipe.read_to_end(&mut buf);
        buf
    });
    let t_err = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = stderr_pipe.read_to_end(&mut buf);
        buf
    });

    let status = loop {
        match child.try_wait().expect("try_wait") {
            Some(st) => break st,
            None => {
                if start.elapsed() >= timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    panic!(
                        "TIMEOUT after {:?} running {:?} with piped stdin — 疑似读取了 stdin 或挂起",
                        timeout, args
                    );
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        }
    };

    let stdout_bytes = t_out.join().expect("join stdout thread");
    let stderr_bytes = t_err.join().expect("join stderr thread");

    RunResult {
        exit_code: status.code().unwrap_or(-1),
        stdout: String::from_utf8(stdout_bytes).expect("stdout must be valid UTF-8"),
        stderr: String::from_utf8_lossy(&stderr_bytes).to_string(),
        elapsed: start.elapsed(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedHumanItem {
    pub outcome: String,
    pub path_or_id: String,
    pub code: Option<String>,
}

/// Parses human-readable item lines to cross-check against JSON output (R17).
pub fn parse_human_items(stdout: &str) -> Vec<ParsedHumanItem> {
    let mut items = Vec::new();
    for line in stdout.lines() {
        let trimmed = line.trim();
        let prefixes = [
            ("[已进回收站]", "succeeded"),
            ("[已恢复]", "succeeded"),
            ("[计划执行]", "planned"),
            ("[被拒绝]", "blocked"),
            ("[失败]", "failed"),
            ("[未执行]", "not_run"),
            ("[无法确认]", "unknown"),
        ];
        for (prefix, outcome) in prefixes {
            if let Some(rest) = trimmed.strip_prefix(prefix) {
                let rest = rest.trim();
                let mut tokens = rest.split_whitespace();
                let path_or_id = tokens.next().unwrap_or("").to_string();
                let code = rest
                    .split_whitespace()
                    .find(|tok| tok.starts_with("E_"))
                    .map(|s| s.to_string());
                items.push(ParsedHumanItem {
                    outcome: outcome.to_string(),
                    path_or_id,
                    code,
                });
                break;
            }
        }
    }
    items
}

// Copyright (C) 2019-2024 Daniel Mueller <deso@posteo.net>
// SPDX-License-Identifier: (Apache-2.0 OR MIT)

//! Query format tests — verify output format correctness.

use std::path::PathBuf;
use std::process::{Command, Stdio};

fn binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_json-test-trace"))
}

fn run_query(input: &str, args: &[&str]) -> (String, String) {
    let mut cmd = Command::new(binary());
    cmd.args(["query", input]);
    cmd.args(args);
    let output = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("failed to run cli");

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    (stdout, stderr)
}

#[test]
fn query_pretty_format_outputs_json_array() {
    let (stdout, _stderr) = run_query("fixtures/mixed_results.jsonl", &["--format", "pretty"]);
    assert!(stdout.trim().starts_with('['), "pretty format should start with [");
    assert!(stdout.trim().ends_with(']'), "pretty format should end with ]");
}

#[test]
fn query_json_format_outputs_jsonl() {
    let (stdout, _stderr) = run_query("fixtures/mixed_results.jsonl", &["--format", "json"]);
    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert!(lines.len() >= 3, "should have at least 3 JSONL lines");
    for line in lines {
        assert!(
            line.starts_with('{'),
            "JSONL line should start with {{: {}",
            line
        );
    }
}

#[test]
fn query_default_format_is_pretty() {
    let (stdout, _stderr) = run_query("fixtures/mixed_results.jsonl", &[]);
    assert!(stdout.starts_with('['), "default format should be pretty (JSON array)");
}
// Copyright (C) 2019-2024 Daniel Mueller <deso@posteo.net>
// SPDX-License-Identifier: (Apache-2.0 OR MIT)

//! Status filter tests — verify pass/fail filtering logic.

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
fn query_status_pass_filters_correctly() {
    let (stdout, _stderr) = run_query(
        "fixtures/mixed_results.jsonl",
        &["--status", "pass", "--format", "json"],
    );
    let lines: Vec<&str> = stdout.trim().lines().collect();
    for line in lines {
        assert!(
            line.contains(r#""status":"PASS""#),
            "pass filter should only include PASS tests: {}",
            line
        );
    }
}

#[test]
fn query_status_fail_filters_correctly() {
    let (stdout, _stderr) = run_query(
        "fixtures/mixed_results.jsonl",
        &["--status", "fail", "--format", "json"],
    );
    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(lines.len(), 1, "should have exactly 1 failed test");
    assert!(lines[0].contains(r#""status":"FAIL""#));
    assert!(lines[0].contains("test_third"));
}

#[test]
fn query_status_pass_includes_only_passed() {
    let (stdout, _stderr) = run_query(
        "fixtures/mixed_results.jsonl",
        &["--status", "pass", "--format", "json"],
    );
    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(lines.len(), 2, "should have exactly 2 passed tests");
    for line in lines {
        let json: serde_json::Value = serde_json::from_str(line).expect("should be valid JSON");
        assert_eq!(json["status"], "PASS");
        assert_eq!(json["has_errors"], false);
    }
}

#[test]
fn query_status_fail_includes_errors() {
    let (stdout, _stderr) = run_query(
        "fixtures/mixed_results.jsonl",
        &["--status", "fail", "--format", "json"],
    );
    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(lines.len(), 1, "should have exactly 1 failed test");
    let json: serde_json::Value = serde_json::from_str(lines[0]).expect("should be valid JSON");
    assert_eq!(json["status"], "FAIL");
    assert_eq!(json["has_errors"], true);
}
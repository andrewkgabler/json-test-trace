// Copyright (C) 2019-2024 Daniel Mueller <deso@posteo.net>
// SPDX-License-Identifier: (Apache-2.0 OR MIT)

//! Status filter tests — verify pass/fail filtering logic.

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use uuid::Uuid;

fn binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_json-test-trace"))
}

fn unique_temp_path(suffix: &str) -> String {
    let id = Uuid::new_v4().to_string();
    format!("/tmp/json-test-trace-qs-{}{}.jsonl", id, suffix)
}

fn run_cargo_test() -> String {
    let jsonl_path = unique_temp_path("");
    let _ = fs::remove_file(&jsonl_path);

    let output = Command::new("cargo")
        .args(["test", "--package", "json-test-trace", "--test", "edge_cases", "--", "--nocapture"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env("TEST_LOG_FILE", &jsonl_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("failed to run cargo test");

    let _stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let _stderr = String::from_utf8_lossy(&output.stderr).to_string();

    fs::read_to_string(&jsonl_path).unwrap_or_default()
}

fn run_query_from_string(content: &str, args: &[&str]) -> (String, String) {
    let jsonl_path = unique_temp_path("-input");
    fs::write(&jsonl_path, content).expect("failed to write temp file");
    
    let mut cmd = Command::new(binary());
    cmd.args(["query", &jsonl_path]);
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
    let jsonl = run_cargo_test();
    let (stdout, _stderr) = run_query_from_string(&jsonl, &["--status", "pass", "--format", "json"]);
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
    let jsonl = run_cargo_test();
    let (stdout, _stderr) = run_query_from_string(&jsonl, &["--status", "fail", "--format", "json"]);
    let lines: Vec<&str> = stdout.trim().lines().collect();
    for line in lines {
        assert!(line.contains(r#""status":"FAIL""#) || line.contains(r#""status":"FAIL (panic)""#));
    }
}

#[test]
fn query_status_pass_includes_only_passed() {
    let jsonl = run_cargo_test();
    let (stdout, _stderr) = run_query_from_string(&jsonl, &["--status", "pass", "--format", "json"]);
    let lines: Vec<&str> = stdout.trim().lines().collect();
    for line in lines {
        let json: serde_json::Value = serde_json::from_str(line).expect("should be valid JSON");
        assert_eq!(json["status"], "PASS");
        assert_eq!(json["has_errors"], false);
    }
}

#[test]
fn query_status_fail_includes_errors() {
    let jsonl = run_cargo_test();
    let (stdout, _stderr) = run_query_from_string(&jsonl, &["--status", "fail", "--format", "json"]);
    let lines: Vec<&str> = stdout.trim().lines().collect();
    for line in lines {
        let json: serde_json::Value = serde_json::from_str(line).expect("should be valid JSON");
        assert!(json["status"].as_str().unwrap_or("").contains("FAIL"));
        assert_eq!(json["has_errors"], true);
    }
}
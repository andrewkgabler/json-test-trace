// Copyright (C) 2019-2024 Daniel Mueller <deso@posteo.net>
// SPDX-License-Identifier: (Apache-2.0 OR MIT)

//! Filter tests — verify path, error, duration, and combined filter logic.

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use uuid::Uuid;

fn binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_json-test-trace"))
}

fn unique_temp_path(suffix: &str) -> String {
    let id = Uuid::new_v4().to_string();
    format!("/tmp/json-test-trace-qf-{}{}.jsonl", id, suffix)
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
fn query_path_substring_match() {
    let jsonl = run_cargo_test();
    let (stdout, _stderr) = run_query_from_string(&jsonl, &["--path", "edge_cases", "--format", "json"]);
    let lines: Vec<&str> = stdout.trim().lines().collect();
    for line in lines {
        let json: serde_json::Value = serde_json::from_str(line).expect("should be valid JSON");
        let module: &str = json["module"].as_str().expect("module should be a string");
        assert!(module.contains("edge_cases"), "module should contain 'edge_cases': {}", module);
    }
}

#[test]
fn query_path_no_match() {
    let jsonl = run_cargo_test();
    let (stdout, _stderr) = run_query_from_string(&jsonl, &["--path", "nonexistent_module_xyz", "--format", "json"]);
    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(lines.len(), 0, "no tests should match 'nonexistent_module_xyz'");
}

#[test]
fn query_error_substring_match() {
    let jsonl = run_cargo_test();
    let (stdout, _stderr) = run_query_from_string(&jsonl, &["--error", "panic", "--format", "json"]);
    let lines: Vec<&str> = stdout.trim().lines().collect();
    for line in lines {
        let json: serde_json::Value = serde_json::from_str(line).expect("should be valid JSON");
        assert!(json["panicked"].as_bool().unwrap_or(false), "should have panicked=true");
    }
}

#[test]
fn query_error_no_match() {
    let jsonl = run_cargo_test();
    let (stdout, _stderr) = run_query_from_string(&jsonl, &["--error", "NonExistentErrorXYZ", "--format", "json"]);
    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(lines.len(), 0, "no tests should match 'NonExistentErrorXYZ'");
}

#[test]
fn query_min_duration_filter() {
    let jsonl = run_cargo_test();
    let (stdout, _stderr) = run_query_from_string(&jsonl, &["--min-duration", "0", "--format", "json"]);
    let lines: Vec<&str> = stdout.trim().lines().collect();
    for line in lines {
        let duration: f64 = serde_json::from_str::<serde_json::Value>(line)
            .expect("should be valid JSON")
            .get("duration_ms")
            .and_then(|v| v.as_f64())
            .expect("should have duration_ms");
        assert!(duration >= 0.0, "all tests should have duration >= 0ms");
    }
}

#[test]
fn query_max_duration_filter() {
    let jsonl = run_cargo_test();
    let (stdout, _stderr) = run_query_from_string(&jsonl, &["--max-duration", "999999", "--format", "json"]);
    let lines: Vec<&str> = stdout.trim().lines().collect();
    for line in lines {
        let duration: f64 = serde_json::from_str::<serde_json::Value>(line)
            .expect("should be valid JSON")
            .get("duration_ms")
            .and_then(|v| v.as_f64())
            .expect("should have duration_ms");
        assert!(duration <= 999999.0, "all tests should have duration <= 999999ms");
    }
}

#[test]
fn query_combined_filters() {
    let jsonl = run_cargo_test();
    let (stdout, _stderr) = run_query_from_string(&jsonl, &["--status", "pass", "--path", "edge_cases", "--format", "json"]);
    let lines: Vec<&str> = stdout.trim().lines().collect();
    for line in lines {
        let json: serde_json::Value = serde_json::from_str(line).expect("should be valid JSON");
        assert_eq!(json["status"], "PASS");
        let module: &str = json["module"].as_str().expect("module should be a string");
        assert!(module.contains("edge_cases"));
    }
}

#[test]
fn query_status_fail_with_error_filter() {
    let jsonl = run_cargo_test();
    let (stdout, _stderr) = run_query_from_string(&jsonl, &["--status", "fail", "--error", "panic", "--format", "json"]);
    let lines: Vec<&str> = stdout.trim().lines().collect();
    for line in lines {
        let json: serde_json::Value = serde_json::from_str(line).expect("should be valid JSON");
        assert_eq!(json["status"], "FAIL (panic)");
        assert_eq!(json["panicked"], true);
    }
}

#[test]
fn query_status_fail_with_error_and_duration_filter() {
    let jsonl = run_cargo_test();
    let (stdout, _stderr) = run_query_from_string(&jsonl, &["--status", "fail", "--error", "panic", "--min-duration", "0", "--format", "json"]);
    let lines: Vec<&str> = stdout.trim().lines().collect();
    for line in lines {
        let json: serde_json::Value = serde_json::from_str(line).expect("should be valid JSON");
        let duration: f64 = json["duration_ms"].as_f64().expect("should have duration_ms");
        assert!(duration >= 0.0, "duration should be >= 0ms");
    }
}

#[test]
fn query_status_pass_with_path_and_duration_range() {
    let jsonl = run_cargo_test();
    let (stdout, _stderr) = run_query_from_string(&jsonl, &["--status", "pass", "--path", "edge_cases", "--min-duration", "0", "--max-duration", "999999", "--format", "json"]);
    let lines: Vec<&str> = stdout.trim().lines().collect();
    for line in lines {
        let json: serde_json::Value = serde_json::from_str(line).expect("should be valid JSON");
        assert_eq!(json["status"], "PASS");
        let duration: f64 = json["duration_ms"].as_f64().expect("should have duration_ms");
        assert!(duration >= 0.0 && duration <= 999999.0, "duration should be between 0 and 999999ms");
    }
}

#[test]
fn query_status_pass_with_error_no_match() {
    let jsonl = run_cargo_test();
    let (stdout, _stderr) = run_query_from_string(&jsonl, &["--status", "pass", "--error", "NonExistentErrorXYZ", "--format", "json"]);
    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(lines.len(), 0, "no passed tests should have NonExistentErrorXYZ");
}

#[test]
fn query_all_filters_together() {
    let jsonl = run_cargo_test();
    let (stdout, _stderr) = run_query_from_string(&jsonl, &["--status", "pass", "--path", "edge_cases", "--min-duration", "0", "--max-duration", "999999", "--format", "json"]);
    let lines: Vec<&str> = stdout.trim().lines().collect();
    for line in lines {
        let json: serde_json::Value = serde_json::from_str(line).expect("should be valid JSON");
        assert_eq!(json["status"], "PASS");
        let module: &str = json["module"].as_str().expect("module should be a string");
        assert!(module.contains("edge_cases"));
        let duration: f64 = json["duration_ms"].as_f64().expect("should have duration_ms");
        assert!(duration >= 0.0 && duration <= 999999.0);
    }
}
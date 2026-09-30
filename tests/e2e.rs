// Copyright (C) 2019-2024 Daniel Mueller <deso@posteo.net>
// SPDX-License-Identifier: (Apache-2.0 OR MIT)

//! E2E tests — real cargo test output piped through CLI.
//!
//! No mock fixtures. Tests generate their own data by running cargo test
//! with json-test-trace, then query the produced JSONL through the CLI.

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use uuid::Uuid;

fn binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_json-test-trace"))
}

fn unique_temp_path(suffix: &str) -> String {
    let id = Uuid::new_v4().to_string();
    format!("/tmp/json-test-trace-e2e-{}{}.jsonl", id, suffix)
}

/// Run cargo test in the json-test-trace crate and return the JSONL content
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

    // Read the JSONL file produced by json-test-trace
    let jsonl = fs::read_to_string(&jsonl_path).unwrap_or_default();

    format!("{}\n{}\n{}", _stdout, _stderr, jsonl)
}

/// Run the CLI query subcommand on a JSONL string
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

/// Run the CLI summary subcommand on a JSONL string
fn run_summary_from_string(content: &str) -> String {
    let jsonl_path = unique_temp_path("-input");
    fs::write(&jsonl_path, content).expect("failed to write temp file");
    
    let output = Command::new(binary())
        .args(["summary", &jsonl_path])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("failed to run cli");

    String::from_utf8_lossy(&output.stdout).to_string()
}

#[test]
fn e2e_cargo_test_produces_jsonl_with_test_records() {
    let output = run_cargo_test();
    // The JSONL content is at the end of the combined output
    // Look for lines that start with { and contain test.name
    let jsonl_lines: Vec<&str> = output.lines()
        .filter(|l| l.starts_with('{') && l.contains("test.name"))
        .collect();
    assert!(jsonl_lines.len() >= 2, "should have at least 2 test records from cargo test, got: {}", jsonl_lines.len());
}

#[test]
fn e2e_query_json_format_from_real_cargo_test() {
    let output = run_cargo_test();
    let jsonl_lines: Vec<&str> = output.lines()
        .filter(|l| l.starts_with('{') && l.contains("test.name"))
        .collect();
    let jsonl_content = jsonl_lines.join("\n");
    
    let (stdout, _stderr) = run_query_from_string(&jsonl_content, &["--format", "json"]);
    let result_lines: Vec<&str> = stdout.trim().lines().collect();
    assert!(result_lines.len() >= 1, "should have at least 1 test result from real cargo test output");
}

#[test]
fn e2e_query_pretty_format_from_real_cargo_test() {
    let output = run_cargo_test();
    let jsonl_lines: Vec<&str> = output.lines()
        .filter(|l| l.starts_with('{') && l.contains("test.name"))
        .collect();
    let jsonl_content = jsonl_lines.join("\n");
    
    let (stdout, _stderr) = run_query_from_string(&jsonl_content, &["--format", "pretty"]);
    assert!(stdout.trim().starts_with('['), "pretty format should start with [");
    assert!(stdout.trim().ends_with(']'), "pretty format should end with ]");
}

#[test]
fn e2e_summary_from_real_cargo_test() {
    let output = run_cargo_test();
    let jsonl_lines: Vec<&str> = output.lines()
        .filter(|l| l.starts_with('{') && l.contains("test.name"))
        .collect();
    let jsonl_content = jsonl_lines.join("\n");
    
    let summary = run_summary_from_string(&jsonl_content);
    let json: serde_json::Value = serde_json::from_str(&summary).expect("summary should be valid JSON");
    let total: usize = json["tests"]["total"].as_u64().expect("total should be a number") as usize;
    assert!(total >= 1, "should have at least 1 test from real cargo test, got: {}", total);
}

#[test]
fn e2e_query_status_pass_from_real_cargo_test() {
    let output = run_cargo_test();
    let jsonl_lines: Vec<&str> = output.lines()
        .filter(|l| l.starts_with('{') && l.contains("test.name"))
        .collect();
    let jsonl_content = jsonl_lines.join("\n");
    
    let (stdout, _stderr) = run_query_from_string(&jsonl_content, &["--status", "pass", "--format", "json"]);
    let result_lines: Vec<&str> = stdout.trim().lines().collect();
    for line in &result_lines {
        let json: serde_json::Value = serde_json::from_str(line).expect("should be valid JSON");
        assert_eq!(json["status"], "PASS", "all tests should be PASS");
    }
}

#[test]
fn e2e_query_default_format_is_pretty() {
    let output = run_cargo_test();
    let jsonl_lines: Vec<&str> = output.lines()
        .filter(|l| l.starts_with('{') && l.contains("test.name"))
        .collect();
    let jsonl_content = jsonl_lines.join("\n");
    
    let (stdout, _stderr) = run_query_from_string(&jsonl_content, &[]);
    assert!(stdout.starts_with('['), "default format should be pretty (JSON array)");
}

#[test]
fn e2e_summary_counts_match_query_results() {
    let output = run_cargo_test();
    let jsonl_lines: Vec<&str> = output.lines()
        .filter(|l| l.starts_with('{') && l.contains("test.name"))
        .collect();
    let jsonl_content = jsonl_lines.join("\n");
    
    let summary = run_summary_from_string(&jsonl_content);
    let summary_json: serde_json::Value = serde_json::from_str(&summary).expect("summary should be valid JSON");
    
    let (query_stdout, _stderr) = run_query_from_string(&jsonl_content, &["--format", "json"]);
    let query_lines: Vec<&str> = query_stdout.trim().lines().collect();
    
    let summary_total: usize = summary_json["tests"]["total"].as_u64().expect("total should be a number") as usize;
    assert_eq!(summary_total, query_lines.len(), "summary total should match query result count");
}

#[test]
fn e2e_query_path_substring_match() {
    let output = run_cargo_test();
    let jsonl_lines: Vec<&str> = output.lines()
        .filter(|l| l.starts_with('{') && l.contains("test.name"))
        .collect();
    let jsonl_content = jsonl_lines.join("\n");
    
    let (stdout, _stderr) = run_query_from_string(&jsonl_content, &["--path", "edge_cases", "--format", "json"]);
    let result_lines: Vec<&str> = stdout.trim().lines().collect();
    for line in &result_lines {
        let json: serde_json::Value = serde_json::from_str(line).expect("should be valid JSON");
        let module: &str = json["module"].as_str().expect("module should be a string");
        assert!(module.contains("edge_cases"), "module should contain 'edge_cases': {}", module);
    }
}

#[test]
fn e2e_query_name_substring_match() {
    let output = run_cargo_test();
    let jsonl_lines: Vec<&str> = output.lines()
        .filter(|l| l.starts_with('{') && l.contains("test.name"))
        .collect();
    let jsonl_content = jsonl_lines.join("\n");
    
    let (stdout, _stderr) = run_query_from_string(&jsonl_content, &["--name", "panic", "--format", "json"]);
    let result_lines: Vec<&str> = stdout.trim().lines().collect();
    for line in &result_lines {
        let json: serde_json::Value = serde_json::from_str(line).expect("should be valid JSON");
        let name: &str = json["name"].as_str().expect("name should be a string");
        assert!(name.contains("panic"), "name should contain 'panic': {}", name);
    }
}

#[test]
fn e2e_summary_includes_modules() {
    let output = run_cargo_test();
    let jsonl_lines: Vec<&str> = output.lines()
        .filter(|l| l.starts_with('{') && l.contains("test.name"))
        .collect();
    let jsonl_content = jsonl_lines.join("\n");
    
    let summary = run_summary_from_string(&jsonl_content);
    let json: serde_json::Value = serde_json::from_str(&summary).expect("summary should be valid JSON");
    let modules: Vec<&str> = json["modules"].as_array().expect("modules should be an array")
        .iter().map(|m| m.as_str().expect("module should be string")).collect();
    assert!(modules.iter().any(|m| m.contains("edge_cases")), "should include edge_cases module");
}

#[test]
fn e2e_query_min_duration_filter() {
    let output = run_cargo_test();
    let jsonl_lines: Vec<&str> = output.lines()
        .filter(|l| l.starts_with('{') && l.contains("test.name"))
        .collect();
    let jsonl_content = jsonl_lines.join("\n");
    
    let (stdout, _stderr) = run_query_from_string(&jsonl_content, &["--min-duration", "0", "--format", "json"]);
    let result_lines: Vec<&str> = stdout.trim().lines().collect();
    for line in &result_lines {
        let json: serde_json::Value = serde_json::from_str(line).expect("should be valid JSON");
        let duration: f64 = json["duration_ms"].as_f64().expect("should have duration_ms");
        assert!(duration >= 0.0, "duration should be >= 0ms");
    }
}

#[test]
fn e2e_query_max_duration_filter() {
    let output = run_cargo_test();
    let jsonl_lines: Vec<&str> = output.lines()
        .filter(|l| l.starts_with('{') && l.contains("test.name"))
        .collect();
    let jsonl_content = jsonl_lines.join("\n");
    
    let (stdout, _stderr) = run_query_from_string(&jsonl_content, &["--max-duration", "999999", "--format", "json"]);
    let result_lines: Vec<&str> = stdout.trim().lines().collect();
    for line in &result_lines {
        let json: serde_json::Value = serde_json::from_str(line).expect("should be valid JSON");
        let duration: f64 = json["duration_ms"].as_f64().expect("should have duration_ms");
        assert!(duration <= 999999.0, "duration should be <= 999999ms");
    }
}

#[test]
fn e2e_query_combined_filters() {
    let output = run_cargo_test();
    let jsonl_lines: Vec<&str> = output.lines()
        .filter(|l| l.starts_with('{') && l.contains("test.name"))
        .collect();
    let jsonl_content = jsonl_lines.join("\n");
    
    let (stdout, _stderr) = run_query_from_string(&jsonl_content, &["--status", "pass", "--path", "edge_cases", "--format", "json"]);
    let result_lines: Vec<&str> = stdout.trim().lines().collect();
    for line in &result_lines {
        let json: serde_json::Value = serde_json::from_str(line).expect("should be valid JSON");
        assert_eq!(json["status"], "PASS");
        let module: &str = json["module"].as_str().expect("module should be a string");
        assert!(module.contains("edge_cases"));
    }
}

#[test]
fn e2e_query_has_errors_field_present() {
    let output = run_cargo_test();
    let jsonl_lines: Vec<&str> = output.lines()
        .filter(|l| l.starts_with('{') && l.contains("test.name"))
        .collect();
    let jsonl_content = jsonl_lines.join("\n");
    
    let (stdout, _stderr) = run_query_from_string(&jsonl_content, &["--format", "json"]);
    let result_lines: Vec<&str> = stdout.trim().lines().collect();
    for line in &result_lines {
        let json: serde_json::Value = serde_json::from_str(line).expect("should be valid JSON");
        assert!(json.get("has_errors").is_some(), "should have has_errors field");
    }
}

#[test]
fn e2e_query_panic_fields_present() {
    let output = run_cargo_test();
    let jsonl_lines: Vec<&str> = output.lines()
        .filter(|l| l.starts_with('{') && l.contains("test.name"))
        .collect();
    let jsonl_content = jsonl_lines.join("\n");
    
    let (stdout, _stderr) = run_query_from_string(&jsonl_content, &["--format", "json"]);
    let result_lines: Vec<&str> = stdout.trim().lines().collect();
    for line in &result_lines {
        let json: serde_json::Value = serde_json::from_str(line).expect("should be valid JSON");
        assert!(json.get("panicked").is_some(), "should have panicked field");
        assert!(json.get("panic_message").is_some(), "should have panic_message field");
    }
}

#[test]
fn e2e_query_ignored_field_present() {
    let output = run_cargo_test();
    let jsonl_lines: Vec<&str> = output.lines()
        .filter(|l| l.starts_with('{') && l.contains("test.name"))
        .collect();
    let jsonl_content = jsonl_lines.join("\n");
    
    let (stdout, _stderr) = run_query_from_string(&jsonl_content, &["--format", "json"]);
    let result_lines: Vec<&str> = stdout.trim().lines().collect();
    for line in &result_lines {
        let json: serde_json::Value = serde_json::from_str(line).expect("should be valid JSON");
        assert!(json.get("ignored").is_some(), "should have ignored field");
    }
}

#[test]
fn e2e_summary_includes_ignored_count() {
    let output = run_cargo_test();
    let jsonl_lines: Vec<&str> = output.lines()
        .filter(|l| l.starts_with('{') && l.contains("test.name"))
        .collect();
    let jsonl_content = jsonl_lines.join("\n");
    
    let summary = run_summary_from_string(&jsonl_content);
    let json: serde_json::Value = serde_json::from_str(&summary).expect("summary should be valid JSON");
    assert!(json["tests"]["ignored"].is_number(), "should have ignored count");
}

#[test]
fn e2e_summary_includes_panicked_count() {
    let output = run_cargo_test();
    let jsonl_lines: Vec<&str> = output.lines()
        .filter(|l| l.starts_with('{') && l.contains("test.name"))
        .collect();
    let jsonl_content = jsonl_lines.join("\n");
    
    let summary = run_summary_from_string(&jsonl_content);
    let json: serde_json::Value = serde_json::from_str(&summary).expect("summary should be valid JSON");
    assert!(json["tests"]["panicked"].is_number(), "should have panicked count");
}
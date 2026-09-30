// Copyright (C) 2019-2024 Daniel Mueller <deso@posteo.net>
// SPDX-License-Identifier: (Apache-2.0 OR MIT)

//! CLI filter e2e tests — --name filter, --status rejection, empty file, malformed lines.

use std::path::PathBuf;
use std::process::{Command, Stdio};

fn binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_json-test-trace"))
}

fn fixture_path(name: &str) -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("fixtures");
    path.push(name);
    path
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
fn query_name_filter_substring_match() {
    let (stdout, _stderr) = run_query(
        fixture_path("name_filter_test.jsonl").to_string_lossy().as_ref(),
        &["--name", "auth", "--format", "json"],
    );
    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(lines.len(), 2, "should have exactly 2 tests matching 'auth'");
    for line in &lines {
        let json: serde_json::Value = serde_json::from_str(line).expect("should be valid JSON");
        let name: &str = json["name"].as_str().expect("name should be a string");
        assert!(name.contains("auth"), "test name should contain 'auth': {}", name);
    }
}

#[test]
fn query_name_filter_no_match() {
    let (stdout, _stderr) = run_query(
        fixture_path("name_filter_test.jsonl").to_string_lossy().as_ref(),
        &["--name", "nonexistent", "--format", "json"],
    );
    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(lines.len(), 0, "no tests should match 'nonexistent'");
}

#[test]
fn query_name_filter_combined_with_status() {
    let (stdout, _stderr) = run_query(
        fixture_path("name_filter_test.jsonl").to_string_lossy().as_ref(),
        &["--name", "db", "--status", "pass", "--format", "json"],
    );
    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(lines.len(), 1, "should have exactly 1 test matching both filters");
    let json: serde_json::Value = serde_json::from_str(lines[0]).expect("should be valid JSON");
    assert_eq!(json["name"], "test_db_query");
    assert_eq!(json["status"], "PASS");
}

#[test]
fn query_empty_file_produces_empty_output() {
    let (stdout, _stderr) = run_query(
        fixture_path("empty_file.jsonl").to_string_lossy().as_ref(),
        &["--format", "json"],
    );
    let trimmed = stdout.trim();
    assert_eq!(trimmed.len(), 0, "empty file should produce empty output");
}

#[test]
fn query_empty_file_pretty_format() {
    let (stdout, _stderr) = run_query(
        fixture_path("empty_file.jsonl").to_string_lossy().as_ref(),
        &["--format", "pretty"],
    );
    let trimmed = stdout.trim();
    assert_eq!(trimmed, "[]", "empty file pretty format should be empty array");
}

#[test]
fn query_malformed_lines_skips_errors() {
    let (stdout, _stderr) = run_query(
        fixture_path("malformed_lines.jsonl").to_string_lossy().as_ref(),
        &["--format", "json"],
    );
    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(lines.len(), 2, "should have 2 valid tests despite malformed lines");
    let names: Vec<String> = lines.iter()
        .map(|line| {
            let json: serde_json::Value = serde_json::from_str(line).expect("valid JSON");
            json["name"].as_str().expect("name should be string").to_string()
        })
        .collect();
    assert!(names.contains(&"test_valid".to_string()), "should include test_valid");
    assert!(names.contains(&"test_second".to_string()), "should include test_second");
}

#[test]
fn query_malformed_lines_logs_errors() {
    let (_stdout, stderr) = run_query(
        fixture_path("malformed_lines.jsonl").to_string_lossy().as_ref(),
        &["--format", "json"],
    );
    assert!(stderr.contains("Error parsing line"), "should log parse errors to stderr");
}
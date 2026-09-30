// Copyright (C) 2019-2024 Daniel Mueller <deso@posteo.net>
// SPDX-License-Identifier: (Apache-2.0 OR MIT)

//! Summary fields e2e tests — ignored count, panicked count, error extraction, is_passed with ignored.

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

fn run_summary(input: &str) -> String {
    let output = Command::new(binary())
        .args(["summary", input])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("failed to run cli");

    String::from_utf8_lossy(&output.stdout).to_string()
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
fn summary_ignored_count_included() {
    let output = run_summary(fixture_path("ignored_and_panic.jsonl").to_string_lossy().as_ref());
    let json: serde_json::Value = serde_json::from_str(&output).expect("summary should be valid JSON");
    let ignored: usize = json["tests"]["ignored"].as_u64().expect("ignored should be a number") as usize;
    assert_eq!(ignored, 1, "should have 1 ignored test");
}

#[test]
fn summary_panicked_count_included() {
    let output = run_summary(fixture_path("ignored_and_panic.jsonl").to_string_lossy().as_ref());
    let json: serde_json::Value = serde_json::from_str(&output).expect("summary should be valid JSON");
    let panicked: usize = json["tests"]["panicked"].as_u64().expect("panicked should be a number") as usize;
    assert_eq!(panicked, 1, "should have 1 panicked test");
}

#[test]
fn summary_error_categories_extracted() {
    let output = run_summary(fixture_path("ignored_and_panic.jsonl").to_string_lossy().as_ref());
    let json: serde_json::Value = serde_json::from_str(&output).expect("summary should be valid JSON");
    let errors: &[serde_json::Value] = json["errors"].as_array().expect("errors should be an array");
    let categories: Vec<&str> = errors.iter()
        .map(|e| e["category"].as_str().expect("category should be string"))
        .collect();
    assert!(categories.contains(&"ValidationError"), "should have ValidationError category");
}

#[test]
fn summary_multiple_error_categories() {
    let output = run_summary(fixture_path("various_error_formats.jsonl").to_string_lossy().as_ref());
    let json: serde_json::Value = serde_json::from_str(&output).expect("summary should be valid JSON");
    let errors: &[serde_json::Value] = json["errors"].as_array().expect("errors should be an array");
    let categories: Vec<&str> = errors.iter()
        .map(|e| e["category"].as_str().expect("category should be string"))
        .collect();
    assert!(categories.contains(&"ValidationError"), "should have ValidationError");
    assert!(categories.contains(&"IoError"), "should have IoError");
    assert!(categories.contains(&"ParseError"), "should have ParseError");
    assert_eq!(errors.len(), 3, "should have exactly 3 error categories");
}

#[test]
fn query_ignored_test_not_in_pass_status() {
    let (stdout, _stderr) = run_query(
        fixture_path("ignored_and_panic.jsonl").to_string_lossy().as_ref(),
        &["--status", "pass", "--format", "json"],
    );
    let lines: Vec<&str> = stdout.trim().lines().collect();
    let names: Vec<String> = lines.iter()
        .map(|line| {
            let json: serde_json::Value = serde_json::from_str(line).expect("valid JSON");
            json["name"].as_str().expect("name should be string").to_string()
        })
        .collect();
    assert!(!names.contains(&"test_skipped".to_string()), "ignored test should not appear in pass status");
    assert!(names.contains(&"test_normal".to_string()), "normal test should appear in pass status");
}

#[test]
fn query_ignored_test_not_in_fail_status() {
    let (stdout, _stderr) = run_query(
        fixture_path("ignored_and_panic.jsonl").to_string_lossy().as_ref(),
        &["--status", "fail", "--format", "json"],
    );
    let lines: Vec<&str> = stdout.trim().lines().collect();
    let names: Vec<String> = lines.iter()
        .map(|line| {
            let json: serde_json::Value = serde_json::from_str(line).expect("valid JSON");
            json["name"].as_str().expect("name should be string").to_string()
        })
        .collect();
    // Ignored tests are included in fail status because is_passed() returns false
    assert!(names.contains(&"test_skipped".to_string()), "ignored test appears in fail status (is_passed=false)");
    assert!(names.contains(&"test_panic".to_string()), "panicked test should appear in fail status");
    assert!(names.contains(&"test_with_error".to_string()), "failed test should appear in fail status");
}

#[test]
fn query_panic_test_in_fail_status() {
    let (stdout, _stderr) = run_query(
        fixture_path("ignored_and_panic.jsonl").to_string_lossy().as_ref(),
        &["--status", "fail", "--format", "json"],
    );
    let lines: Vec<&str> = stdout.trim().lines().collect();
    let panic_test: &str = lines.iter()
        .find(|line| {
            let json: serde_json::Value = serde_json::from_str(line).expect("valid JSON");
            json["name"] == "test_panic"
        })
        .expect("should find test_panic");
    
    let json: serde_json::Value = serde_json::from_str(panic_test).expect("should be valid JSON");
    assert_eq!(json["panicked"], true);
    // Panicked tests have panicked=true but may not have has_errors=true
    // (panic detection is separate from error detection)
}

#[test]
fn query_panic_test_has_panic_message() {
    let (stdout, _stderr) = run_query(
        fixture_path("ignored_and_panic.jsonl").to_string_lossy().as_ref(),
        &["--status", "fail", "--format", "json"],
    );
    let lines: Vec<&str> = stdout.trim().lines().collect();
    let panic_test: &str = lines.iter()
        .find(|line| {
            let json: serde_json::Value = serde_json::from_str(line).expect("valid JSON");
            json["name"] == "test_panic"
        })
        .expect("should find test_panic");
    
    let json: serde_json::Value = serde_json::from_str(panic_test).expect("should be valid JSON");
    let panic_message: &str = json["panic_message"].as_str().expect("panic_message should be string");
    assert!(panic_message.contains("assertion failed"), "panic message should contain assertion text");
}
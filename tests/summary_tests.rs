// Copyright (C) 2019-2024 Daniel Mueller <deso@posteo.net>
// SPDX-License-Identifier: (Apache-2.0 OR MIT)

//! Summary tests — verify summary output structure and content.

use std::path::PathBuf;
use std::process::{Command, Stdio};

fn binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_json-test-trace"))
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

#[test]
fn summary_produces_valid_json() {
    let output = run_summary("fixtures/mixed_results.jsonl");
    let json: serde_json::Value = serde_json::from_str(&output).expect("summary should be valid JSON");
    assert_eq!(json["tests"]["total"], 3);
    assert_eq!(json["tests"]["passed"], 2);
    assert_eq!(json["tests"]["failed"], 1);
}

#[test]
fn summary_includes_error_categories() {
    let output = run_summary("fixtures/mixed_results.jsonl");
    let json: serde_json::Value = serde_json::from_str(&output).expect("summary should be valid JSON");
    let errors = json["errors"].as_array().expect("errors should be an array");
    assert!(errors.len() >= 1, "should have at least 1 error category");
    let validation_error = errors
        .iter()
        .find(|e| e["category"] == "ValidationError")
        .expect("should have ValidationError category");
    assert_eq!(validation_error["count"], 1);
}

#[test]
fn summary_includes_modules() {
    let output = run_summary("fixtures/mixed_results.jsonl");
    let json: serde_json::Value = serde_json::from_str(&output).expect("summary should be valid JSON");
    let modules = json["modules"].as_array().expect("modules should be an array");
    assert!(
        modules.iter().any(|m| m == "allograph::auth::claims"),
        "should include module name"
    );
}
// Copyright (C) 2019-2024 Daniel Mueller <deso@posteo.net>
// SPDX-License-Identifier: (Apache-2.0 OR MIT)

//! Summary tests — verify summary output structure and content.

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use uuid::Uuid;

fn binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_json-test-trace"))
}

fn unique_temp_path(suffix: &str) -> String {
    let id = Uuid::new_v4().to_string();
    format!("/tmp/json-test-trace-st-{}{}.jsonl", id, suffix)
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
fn summary_produces_valid_json() {
    let jsonl = run_cargo_test();
    let output = run_summary_from_string(&jsonl);
    let json: serde_json::Value = serde_json::from_str(&output).expect("summary should be valid JSON");
    let total: usize = json["tests"]["total"].as_u64().expect("total should be a number") as usize;
    assert!(total >= 1, "should have at least 1 test from real cargo test");
}

#[test]
fn summary_includes_error_categories() {
    let jsonl = run_cargo_test();
    let output = run_summary_from_string(&jsonl);
    let json: serde_json::Value = serde_json::from_str(&output).expect("summary should be valid JSON");
    let errors = json["errors"].as_array().expect("errors should be an array");
    for error in errors {
        assert!(error["category"].is_string(), "category should be a string");
        assert!(error["count"].is_number(), "count should be a number");
    }
}

#[test]
fn summary_includes_modules() {
    let jsonl = run_cargo_test();
    let output = run_summary_from_string(&jsonl);
    let json: serde_json::Value = serde_json::from_str(&output).expect("summary should be valid JSON");
    let modules = json["modules"].as_array().expect("modules should be an array");
    assert!(
        modules.iter().any(|m| m.as_str().unwrap_or("").contains("edge_cases")),
        "should include edge_cases module"
    );
}
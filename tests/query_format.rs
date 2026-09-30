// Copyright (C) 2019-2024 Daniel Mueller <deso@posteo.net>
// SPDX-License-Identifier: (Apache-2.0 OR MIT)

//! Query format tests — verify output format correctness.

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
fn query_pretty_format_outputs_json_array() {
    let jsonl = run_cargo_test();
    let (stdout, _stderr) = run_query_from_string(&jsonl, &["--format", "pretty"]);
    assert!(stdout.trim().starts_with('['), "pretty format should start with [");
    assert!(stdout.trim().ends_with(']'), "pretty format should end with ]");
}

#[test]
fn query_json_format_outputs_jsonl() {
    let jsonl = run_cargo_test();
    let (stdout, _stderr) = run_query_from_string(&jsonl, &["--format", "json"]);
    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert!(lines.len() >= 1, "should have at least 1 JSONL line");
    for line in lines {
        assert!(line.starts_with('{'), "JSONL line should start with {{: {}", line);
    }
}

#[test]
fn query_default_format_is_pretty() {
    let jsonl = run_cargo_test();
    let (stdout, _stderr) = run_query_from_string(&jsonl, &[]);
    assert!(stdout.starts_with('['), "default format should be pretty (JSON array)");
}
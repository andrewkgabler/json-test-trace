// Copyright (C) 2019-2024 Daniel Mueller <deso@posteo.net>
// SPDX-License-Identifier: (Apache-2.0 OR MIT)

//! Instructions tests — verify instructions output structure and content.

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use uuid::Uuid;

fn binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_json-test-trace"))
}

fn unique_temp_path(suffix: &str) -> String {
    let id = Uuid::new_v4().to_string();
    format!("/tmp/json-test-trace-it-{}{}.jsonl", id, suffix)
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

fn run_instructions_from_string(content: &str) -> String {
    let jsonl_path = unique_temp_path("-input");
    fs::write(&jsonl_path, content).expect("failed to write temp file");
    
    let output = Command::new(binary())
        .args(["instructions", &jsonl_path])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("failed to run cli");

    String::from_utf8_lossy(&output.stdout).to_string()
}

#[test]
fn instructions_produces_readable_output() {
    let jsonl = run_cargo_test();
    let output = run_instructions_from_string(&jsonl);
    let header_count = output.matches("===").count();
    assert!(header_count >= 2, "should have at least 2 test headers (opening and closing ===), got: {}", header_count);
}

#[test]
fn instructions_shows_errors_for_failed_tests() {
    let jsonl = run_cargo_test();
    let output = run_instructions_from_string(&jsonl);
    assert!(output.contains("Status:"), "instructions should show Status:");
}
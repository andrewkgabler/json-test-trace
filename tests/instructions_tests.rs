// Copyright (C) 2019-2024 Daniel Mueller <deso@posteo.net>
// SPDX-License-Identifier: (Apache-2.0 OR MIT)

//! Instructions tests — verify instructions output structure and content.

use std::path::PathBuf;
use std::process::{Command, Stdio};

fn binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_json-test-trace"))
}

fn run_instructions(input: &str) -> String {
    let output = Command::new(binary())
        .args(["instructions", input])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("failed to run cli");

    String::from_utf8_lossy(&output.stdout).to_string()
}

#[test]
fn instructions_produces_readable_output() {
    let output = run_instructions("fixtures/mixed_results.jsonl");
    assert!(output.contains("=== test_first ==="));
    assert!(output.contains("=== test_second ==="));
    assert!(output.contains("=== test_third ==="));
    assert!(output.contains("Status: PASS"));
    assert!(output.contains("Status: FAIL"));
}

#[test]
fn instructions_shows_errors_for_failed_tests() {
    let output = run_instructions("fixtures/mixed_results.jsonl");
    assert!(
        output.contains("Errors: Test failed with errors in log"),
        "instructions should show error indicator for failed tests"
    );
}
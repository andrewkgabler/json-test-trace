// Copyright (C) 2019-2024 Daniel Mueller <deso@posteo.net>
// SPDX-License-Identifier: (Apache-2.0 OR MIT)

//! Error handling tests — verify error cases and edge conditions.

use std::path::PathBuf;
use std::process::{Command, Stdio};

fn binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_json-test-trace"))
}

#[test]
fn query_file_not_found() {
    let output = Command::new(binary())
        .args(["query", "/tmp/nonexistent_file_12345.jsonl"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("failed to run cli");

    assert!(!output.status.success(), "should fail for nonexistent file");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Error reading file"), "should show error message");
}
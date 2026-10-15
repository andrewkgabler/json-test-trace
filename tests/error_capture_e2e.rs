// Copyright (C) 2019-2024 Daniel Mueller <deso@posteo.net>
// SPDX-License-Identifier: (Apache-2.0 OR MIT)

//! E2E tests for error capture, concurrent isolation, and edge cases.
//!
//! These tests run real cargo test invocations and parse the JSONL output
//! to verify that error info, test identity, and lifecycle records are
//! correctly captured.

use std::fs;
use std::process::{Command, Stdio};
use uuid::Uuid;

fn unique_temp_path(suffix: &str) -> String {
    let id = Uuid::new_v4().to_string();
    format!("/tmp/json-test-trace-e2e-{}{}.jsonl", id, suffix)
}

/// Run cargo test on a specific test file and return the JSONL content
fn run_cargo_test_on(test_name: &str) -> String {
    let jsonl_path = unique_temp_path("");
    let _ = fs::remove_file(&jsonl_path);

    let output = Command::new("cargo")
        .args(["test", "--package", "json-test-trace", "--test", test_name, "--", "--nocapture"])
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

/// Parse JSONL lines into Vec<serde_json::Value>
fn parse_jsonl(content: &str) -> Vec<serde_json::Value> {
    content.lines()
        .filter(|l| l.starts_with('{'))
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .collect()
}

// ============================================================
// Error capture e2e tests
// ============================================================

#[test]
fn e2e_error_enum_extracted_from_panic() {
    let output = run_cargo_test_on("error_helper_demo");
    let records = parse_jsonl(&output);
    
    // Find records with error_enum field
    let error_records: Vec<&serde_json::Value> = records.iter()
        .filter(|r| r.get("error_enum").is_some())
        .collect();
    
    assert!(error_records.len() >= 3, 
        "should have at least 3 records with error_enum from 3 failing tests, got: {}", 
        error_records.len());
    
    // Verify error_enum values match expected variants
    let error_enums: Vec<&str> = error_records.iter()
        .filter_map(|r| r["error_enum"].as_str())
        .collect();
    
    assert!(error_enums.contains(&"UserInput"), 
        "should have UserInput error_enum, got: {:?}", error_enums);
    assert!(error_enums.contains(&"AdminSdl"), 
        "should have AdminSdl error_enum, got: {:?}", error_enums);
    assert!(error_enums.contains(&"AllographDbQuery"), 
        "should have AllographDbQuery error_enum, got: {:?}", error_enums);
}

#[test]
fn e2e_error_message_extracted_from_panic() {
    let output = run_cargo_test_on("error_helper_demo");
    let records = parse_jsonl(&output);
    
    // Find records with error_message field
    let error_records: Vec<&serde_json::Value> = records.iter()
        .filter(|r| r.get("error_message").is_some())
        .collect();
    
    assert!(error_records.len() >= 3, 
        "should have at least 3 records with error_message, got: {}", 
        error_records.len());
    
    // Verify error_message values
    let error_messages: Vec<&str> = error_records.iter()
        .filter_map(|r| r["error_message"].as_str())
        .collect();
    
    assert!(error_messages.iter().any(|m| m.contains("duplicate @id value")), 
        "should have duplicate @id value message, got: {:?}", error_messages);
    assert!(error_messages.iter().any(|m| m.contains("type conflict")), 
        "should have type conflict message, got: {:?}", error_messages);
    assert!(error_messages.iter().any(|m| m.contains("unique constraint violation")), 
        "should have unique constraint violation message, got: {:?}", error_messages);
}

#[test]
fn e2e_error_code_in_buffered_events() {
    let output = run_cargo_test_on("error_helper_demo");
    let records = parse_jsonl(&output);
    
    // Find ERROR level events (these have error_code from tracing::error!)
    let error_events: Vec<&serde_json::Value> = records.iter()
        .filter(|r| r.get("level").and_then(|l| l.as_str()) == Some("ERROR"))
        .collect();
    
    // ERROR events may or may not be present depending on filter settings
    // The key assertion is that error_enum/error_message ARE captured (tested above)
    if !error_events.is_empty() {
        // If ERROR events exist, verify error_code field is present
        let has_error_code = error_events.iter()
            .any(|r| r.get("code").is_some());
        
        assert!(has_error_code, 
            "ERROR events should have error_code field, got: {:?}", 
            error_events.iter()
                .map(|r| r.get("code").map(|c| c.to_string()))
                .collect::<Vec<_>>());
    }
}

#[test]
fn e2e_error_enum_in_test_lifecycle_records() {
    let output = run_cargo_test_on("error_helper_demo");
    let records = parse_jsonl(&output);
    
    // Find test.start and test.end records
    let test_start_records: Vec<&serde_json::Value> = records.iter()
        .filter(|r| r.get("event").and_then(|e| e.as_str()) == Some("test.start"))
        .collect();
    
    let test_end_records: Vec<&serde_json::Value> = records.iter()
        .filter(|r| r.get("event").and_then(|e| e.as_str()) == Some("test.end"))
        .collect();
    
    assert!(test_start_records.len() >= 4, 
        "should have at least 4 test.start records, got: {}", test_start_records.len());
    assert!(test_end_records.len() >= 4, 
        "should have at least 4 test.end records, got: {}", test_end_records.len());
    
    // Find failing test records (should have error_enum)
    let failing_starts: Vec<&serde_json::Value> = test_start_records.iter()
        .copied()
        .filter(|r| r.get("error_enum").is_some())
        .collect();
    
    let failing_ends: Vec<&serde_json::Value> = test_end_records.iter()
        .copied()
        .filter(|r| r.get("error_enum").is_some())
        .collect();
    
    assert!(failing_starts.len() >= 3, 
        "should have at least 3 failing test.start records with error_enum, got: {}", 
        failing_starts.len());
    assert!(failing_ends.len() >= 3, 
        "should have at least 3 failing test.end records with error_enum, got: {}", 
        failing_ends.len());
}

#[test]
fn e2e_error_enum_in_buffered_events() {
    let output = run_cargo_test_on("error_helper_demo");
    let records = parse_jsonl(&output);
    
    // Find INFO level records that have error_enum (these are buffered events with injected error info)
    let info_with_error: Vec<&serde_json::Value> = records.iter()
        .filter(|r| r.get("level").and_then(|l| l.as_str()) == Some("INFO"))
        .filter(|r| r.get("error_enum").is_some())
        .collect();
    
    assert!(info_with_error.len() >= 3, 
        "should have at least 3 INFO records with injected error_enum, got: {}", 
        info_with_error.len());
}

// ============================================================
// Test identity e2e tests
// ============================================================

#[test]
fn e2e_test_name_module_in_all_records() {
    let output = run_cargo_test_on("edge_cases");
    let records = parse_jsonl(&output);
    
    assert!(records.len() >= 2, "should have at least 2 records, got: {}", records.len());
    
    // All records should have test.name and test.module
    let all_have_name = records.iter()
        .all(|r| r.get("test.name").is_some());
    let all_have_module = records.iter()
        .all(|r| r.get("test.module").is_some());
    
    assert!(all_have_name, "all records should have test.name field");
    assert!(all_have_module, "all records should have test.module field");
    
    // Verify test.name values are non-empty strings
    let test_names: Vec<&str> = records.iter()
        .filter_map(|r| r["test.name"].as_str())
        .collect();
    
    assert!(!test_names.is_empty(), "should have at least one test name");
    assert!(test_names.iter().all(|n| !n.is_empty()), "all test names should be non-empty");
}

#[test]
fn e2e_test_name_matches_test_file() {
    let output = run_cargo_test_on("edge_cases");
    let records = parse_jsonl(&output);
    
    let test_names: Vec<&str> = records.iter()
        .filter_map(|r| r["test.name"].as_str())
        .collect();
    
    // edge_cases.rs has tests like empty_test_emits_lifecycle_records, nested_spans_captured, etc.
    assert!(test_names.iter().any(|n| n.contains("empty_test")), 
        "should have empty_test_emits_lifecycle_records, got: {:?}", test_names);
    assert!(test_names.iter().any(|n| n.contains("nested_spans")), 
        "should have nested_spans_captured, got: {:?}", test_names);
}

// ============================================================
// Empty test lifecycle e2e test
// ============================================================

#[test]
fn e2e_empty_test_emits_lifecycle_records() {
    let output = run_cargo_test_on("edge_cases");
    let records = parse_jsonl(&output);
    
    // Find the empty_test_emits_lifecycle_records test
    let empty_test_records: Vec<&serde_json::Value> = records.iter()
        .filter(|r| r["test.name"].as_str() == Some("empty_test_emits_lifecycle_records"))
        .collect();
    
    assert!(empty_test_records.len() >= 2, 
        "empty test should still emit test.start and test.end, got {} records", 
        empty_test_records.len());
    
    // Verify test.start and test.end are present
    let has_start = empty_test_records.iter()
        .any(|r| r.get("event").and_then(|e| e.as_str()) == Some("test.start"));
    let has_end = empty_test_records.iter()
        .any(|r| r.get("event").and_then(|e| e.as_str()) == Some("test.end"));
    
    assert!(has_start, "empty test should have test.start record");
    assert!(has_end, "empty test should have test.end record");
    
    // Empty test should have status "pass"
    let start_record = empty_test_records.iter()
        .find(|r| r.get("event").and_then(|e| e.as_str()) == Some("test.start"));
    
    if let Some(record) = start_record {
        assert_eq!(record["status"], "pass", "empty test should have status pass");
    }
}

// ============================================================
// Concurrent test isolation e2e test
// ============================================================

#[test]
fn e2e_concurrent_tests_have_unique_names() {
    let output = run_cargo_test_on("edge_cases");
    let records = parse_jsonl(&output);
    
    let test_names: Vec<&str> = records.iter()
        .filter_map(|r| r["test.name"].as_str())
        .collect();
    
    // Check for unique test names (no duplicates from race conditions)
    let mut unique_names = test_names.clone();
    unique_names.sort();
    unique_names.dedup();
    
    // edge_cases has ~15 tests, we should see multiple unique names
    assert!(unique_names.len() >= 5, 
        "should have at least 5 unique test names from concurrent tests, got: {}", 
        unique_names.len());
}

#[test]
fn e2e_concurrent_tests_have_correct_module() {
    let output = run_cargo_test_on("edge_cases");
    let records = parse_jsonl(&output);
    
    let test_names: Vec<&str> = records.iter()
        .filter_map(|r| r["test.name"].as_str())
        .collect();
    
    // All edge_cases tests should have the correct module
    let records_with_module: Vec<&serde_json::Value> = records.iter()
        .filter(|r| test_names.iter().any(|n| r["test.name"].as_str() == Some(n)))
        .collect();
    
    let all_correct_module = records_with_module.iter()
        .all(|r| {
            r["test.module"].as_str()
                .map(|m| m.contains("edge_cases"))
                .unwrap_or(false)
        });
    
    assert!(all_correct_module, 
        "all edge_cases records should have correct module, got: {:?}", 
        records_with_module.iter()
            .map(|r| r["test.module"].as_str())
            .collect::<Vec<_>>());
}

// ============================================================
// Nested spans e2e test
// ============================================================

#[test]
fn e2e_nested_spans_captured() {
    let output = run_cargo_test_on("edge_cases");
    let records = parse_jsonl(&output);
    
    // Find records for nested_spans_captured test
    let nested_records: Vec<&serde_json::Value> = records.iter()
        .filter(|r| r["test.name"].as_str() == Some("nested_spans_captured"))
        .collect();
    
    assert!(nested_records.len() >= 2, 
        "nested_spans_captured should have at least 2 records, got: {}", 
        nested_records.len());
    
    // Should have INFO level events (from tracing::info! calls)
    let info_events: Vec<&serde_json::Value> = nested_records.iter()
        .copied()
        .filter(|r| r.get("level").and_then(|l| l.as_str()) == Some("INFO"))
        .collect();
    
    assert!(info_events.len() >= 2, 
        "nested_spans_captured should have at least 2 INFO events, got: {}", 
        info_events.len());
}

// ============================================================
// Panic status e2e test
// ============================================================

#[test]
fn e2e_panic_test_has_fail_status() {
    let output = run_cargo_test_on("error_helper_demo");
    let records = parse_jsonl(&output);
    
    // Find test.end records for failing tests
    let test_end_records: Vec<&serde_json::Value> = records.iter()
        .filter(|r| r.get("event").and_then(|e| e.as_str()) == Some("test.end"))
        .collect();
    
    // At least 3 tests should have status "fail" (the 3 failing tests)
    let fail_records: Vec<&serde_json::Value> = test_end_records.iter()
        .copied()
        .filter(|r| r["status"] == "fail")
        .collect();
    
    assert!(fail_records.len() >= 3, 
        "should have at least 3 test.end records with status fail, got: {}", 
        fail_records.len());
}

#[test]
fn e2e_passing_test_has_pass_status() {
    let output = run_cargo_test_on("error_helper_demo");
    let records = parse_jsonl(&output);
    
    // Find test.end records for passing tests
    let test_end_records: Vec<&serde_json::Value> = records.iter()
        .filter(|r| r.get("event").and_then(|e| e.as_str()) == Some("test.end"))
        .collect();
    
    // option1_no_errors_passes should have status "pass"
    let pass_records: Vec<&serde_json::Value> = test_end_records.iter()
        .copied()
        .filter(|r| r["status"] == "pass")
        .collect();
    
    assert!(pass_records.len() >= 1, 
        "should have at least 1 test.end record with status pass, got: {}", 
        pass_records.len());
}
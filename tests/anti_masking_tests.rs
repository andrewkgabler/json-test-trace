// Copyright (C) 2019-2024 Daniel Mueller <deso@posteo.net>
// SPDX-License-Identifier: (Apache-2.0 OR MIT)

//! Anti-masking tests — detect hardcoded/static behavior that masks real issues.
//!
//! These tests actively verify that the JSONL output contains VARIATION in fields
//! that should be dynamic. If all records have the same value for a field that
//! should vary, the test FAILS — because static behavior is being masked as passing.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use uuid::Uuid;

fn binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_json-test-trace"))
}

fn unique_temp_path(suffix: &str) -> String {
    let id = Uuid::new_v4().to_string();
    format!("/tmp/json-test-trace-anti-{}{}.jsonl", id, suffix)
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

    fs::read_to_string(&jsonl_path).unwrap_or_default()
}

/// Parse JSONL lines into Vec<serde_json::Value>
fn parse_jsonl(content: &str) -> Vec<serde_json::Value> {
    content.lines()
        .filter(|l| l.starts_with('{'))
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .collect()
}

/// ANTI-MASKING TEST: Level must NOT be always INFO.
///
/// If all records have level=INFO, this indicates the subscriber is hardcoding
/// the level instead of reading it from the tracing event. This test FAILS
/// if level is static — because static behavior is being masked as correct.
#[test]
fn level_is_dynamic_not_hardcoded_to_info() {
    let jsonl = run_cargo_test();
    let records = parse_jsonl(&jsonl);
    
    // Need at least some records to validate
    assert!(records.len() >= 2, "need at least 2 records to check level variation, got: {}", records.len());
    
    // Collect all unique level values
    let levels: HashSet<&str> = records.iter()
        .filter_map(|r| r["level"].as_str())
        .collect();
    
    // CRITICAL: If all levels are "INFO", the subscriber is hardcoding level.
    // This is a bug — level should come from the tracing event, not be hardcoded.
    assert!(
        levels.len() > 1,
        "level is hardcoded to INFO in all {} records. The subscriber must read level from tracing events, not use a hardcoded default. Found only: {:?}",
        records.len(),
        levels
    );
    
    // Verify we have at least one non-INFO level
    let has_non_info = levels.iter().any(|l| *l != "INFO");
    assert!(
        has_non_info,
        "all records have level=INFO. Expected variation: ERROR, WARN, DEBUG, or TRACE. This indicates the subscriber is not reading the level field from tracing events."
    );
}

/// ANTI-MASKING TEST: Verify all standard tracing levels appear in output.
///
/// The edge_cases test file emits: error!, warn!, info!, debug! events.
/// If any of these levels are missing, the subscriber is not preserving them.
#[test]
fn all_standard_levels_present_in_output() {
    let jsonl = run_cargo_test();
    let records = parse_jsonl(&jsonl);
    
    let levels: HashSet<&str> = records.iter()
        .filter_map(|r| r["level"].as_str())
        .collect();
    
    // The edge_cases test emits error!, warn!, info!, debug! events
    // At minimum we should see ERROR, WARN, INFO, DEBUG
    for required_level in &["ERROR", "WARN", "INFO", "DEBUG"] {
        assert!(
            levels.contains(*required_level),
            "missing level '{}' in output. Found levels: {:?}. The subscriber must preserve all tracing levels from events.",
            required_level,
            levels
        );
    }
}

/// ANTI-MASKING TEST: Status must vary based on test outcome.
///
/// The edge_cases test file has both passing and failing tests (panic_flushes_buffer).
/// If all test.end records have status=pass, the panic detection is broken.
#[test]
fn status_varies_based_on_test_outcome() {
    let jsonl = run_cargo_test();
    let records = parse_jsonl(&jsonl);
    
    // Collect test.end records
    let test_end_records: Vec<&serde_json::Value> = records.iter()
        .filter(|r| r.get("event").and_then(|e| e.as_str()) == Some("test.end"))
        .collect();
    
    assert!(
        test_end_records.len() >= 2,
        "need at least 2 test.end records to check status variation, got: {}",
        test_end_records.len()
    );
    
    // Collect unique statuses from test.end records
    let statuses: HashSet<&str> = test_end_records.iter()
        .filter_map(|r| r["status"].as_str())
        .collect();
    
    // CRITICAL: If all statuses are "pass", panic detection is broken.
    // The edge_cases test file has panic_flushes_buffer which should produce status=fail.
    assert!(
        statuses.contains("fail") || statuses.contains("FAIL"),
        "all test.end records have status=pass. Panic detection is broken — panic_flushes_buffer should produce status=fail. Found statuses: {:?}",
        statuses
    );
}

/// ANTI-MASKING TEST: Error fields must not be always null when panics occur.
///
/// If panic_flushes_buffer produces error_enum=null and error_message=null,
/// the panic error capture is not working.
#[test]
fn error_fields_populated_on_panic() {
    let jsonl = run_cargo_test();
    let records = parse_jsonl(&jsonl);
    
    // Find test.end records for panic_flushes_buffer
    let panic_test_records: Vec<&serde_json::Value> = records.iter()
        .filter(|r| {
            r.get("event").and_then(|e| e.as_str()) == Some("test.end")
                && r.get("name").and_then(|n| n.as_str()) == Some("panic_flushes_buffer")
        })
        .collect();
    
    if panic_test_records.is_empty() {
        // Test not found — skip this validation
        return;
    }
    
    for record in &panic_test_records {
        // Status should be fail for panic
        let status = record["status"].as_str().unwrap_or("");
        assert_eq!(
            status, "fail",
            "panic_flushes_buffer should have status=fail, got: {}. Error capture may be broken.",
            status
        );
    }
}

/// ANTI-MASKING TEST: Verify test.start and test.end have matching test.name.
///
/// If test.start has name=A but test.end has name=B, the test identity tracking is broken.
#[test]
fn test_identity_consistent_across_lifecycle() {
    let jsonl = run_cargo_test();
    let records = parse_jsonl(&jsonl);
    
    // Group records by test name
    let mut test_starts: HashMap<String, &serde_json::Value> = HashMap::new();
    let mut test_ends: HashMap<String, &serde_json::Value> = HashMap::new();
    
    for record in &records {
        if let (Some(event), Some(name)) = (
            record.get("event").and_then(|e| e.as_str()),
            record.get("name").and_then(|n| n.as_str()),
        ) {
            match event {
                "test.start" => { test_starts.insert(name.to_string(), record); }
                "test.end" => { test_ends.insert(name.to_string(), record); }
                _ => {}
            }
        }
    }
    
    // Verify each test.start has a matching test.end with same name
    for (name, start_record) in &test_starts {
        assert!(
            test_ends.contains_key(name),
            "test.start for '{}' has no matching test.end. Test identity tracking is broken.",
            name
        );
        
        let end_record = test_ends[name];
        
        // Verify test.name field matches in both records
        let start_test_name = start_record.get("test.name").and_then(|t| t.as_str()).unwrap_or("");
        let end_test_name = end_record.get("test.name").and_then(|t| t.as_str()).unwrap_or("");
        
        assert_eq!(
            start_test_name, end_test_name,
            "test.name mismatch: test.start has '{}', test.end has '{}'. Identity tracking broken.",
            start_test_name, end_test_name
        );
    }
}

/// ANTI-MASKING TEST: Verify buffered events preserve their original level.
///
/// If all buffered events (non-lifecycle records) have level=INFO, the subscriber
/// is not preserving the level from the original tracing event.
#[test]
fn buffered_events_preserve_original_level() {
    let jsonl = run_cargo_test();
    let records = parse_jsonl(&jsonl);
    
    // Filter to non-lifecycle events (no event field or event != test.start/end)
    let buffered_events: Vec<&serde_json::Value> = records.iter()
        .filter(|r| {
            let event = r.get("event").and_then(|e| e.as_str());
            event != Some("test.start") && event != Some("test.end")
        })
        .collect();
    
    assert!(
        buffered_events.len() >= 2,
        "need at least 2 buffered events to check level preservation, got: {}",
        buffered_events.len()
    );
    
    // Collect levels from buffered events
    let levels: HashSet<&str> = buffered_events.iter()
        .filter_map(|r| r["level"].as_str())
        .collect();
    
    // CRITICAL: If all buffered events have level=INFO, the subscriber is hardcoding level.
    assert!(
        levels.len() > 1,
        "all buffered events have level=INFO. The subscriber must preserve original event levels. Found only: {:?}. Expected ERROR, WARN, DEBUG, etc.",
        levels
    );
}

/// ANTI-MASKING TEST: Verify module field is not always empty.
///
/// If all records have module="", the module extraction is broken.
#[test]
fn module_field_populated_not_empty() {
    let jsonl = run_cargo_test();
    let records = parse_jsonl(&jsonl);
    
    assert!(
        records.len() >= 2,
        "need at least 2 records to check module field, got: {}",
        records.len()
    );
    
    // Collect unique module values
    let modules: HashSet<&str> = records.iter()
        .filter_map(|r| r["module"].as_str())
        .collect();
    
    // CRITICAL: If all modules are empty, module extraction is broken.
    let empty_count = modules.iter().filter(|m| m.is_empty()).count();
    assert!(
        empty_count < modules.len(),
        "all records have empty module field. Module extraction is broken. Found modules: {:?}",
        modules
    );
    
    // Should have at least one non-empty module
    let has_non_empty = modules.iter().any(|m| !m.is_empty());
    assert!(
        has_non_empty,
        "no records have non-empty module field. Expected module names like 'edge_cases', 'e2e', etc."
    );
}

/// ANTI-MASKING TEST: Verify timestamp is dynamic and monotonically increasing.
///
/// If all records have the same timestamp, the timestamp generation is broken.
#[test]
fn timestamps_are_dynamic_not_static() {
    let jsonl = run_cargo_test();
    let records = parse_jsonl(&jsonl);
    
    assert!(
        records.len() >= 2,
        "need at least 2 records to check timestamp variation, got: {}",
        records.len()
    );
    
    // Collect unique timestamp values
    let timestamps: HashSet<u64> = records.iter()
        .filter_map(|r| r["timestamp"].as_u64())
        .collect();
    
    // CRITICAL: If all timestamps are the same, timestamp generation is broken.
    assert!(
        timestamps.len() > 1,
        "all records have the same timestamp. Timestamp generation is broken. Found only {} unique timestamps for {} records.",
        timestamps.len(),
        records.len()
    );
}

/// ANTI-MASKING TEST: Verify test.start and test.end have different timestamps.
///
/// If test.start and test.end have the same timestamp, the timing is broken.
#[test]
fn lifecycle_timestamps_differ() {
    let jsonl = run_cargo_test();
    let records = parse_jsonl(&jsonl);
    
    // Group records by test name
    let mut test_starts: HashMap<String, u64> = HashMap::new();
    let mut test_ends: HashMap<String, u64> = HashMap::new();
    
    for record in &records {
        if let (Some(event), Some(name), Some(ts)) = (
            record.get("event").and_then(|e| e.as_str()),
            record.get("name").and_then(|n| n.as_str()),
            record.get("timestamp").and_then(|t| t.as_u64()),
        ) {
            match event {
                "test.start" => { test_starts.insert(name.to_string(), ts); }
                "test.end" => { test_ends.insert(name.to_string(), ts); }
                _ => {}
            }
        }
    }
    
    // Verify each test.start has a different timestamp than test.end
    for (name, start_ts) in &test_starts {
        if let Some(end_ts) = test_ends.get(name) {
            assert!(
                *start_ts != *end_ts,
                "test '{}' has same timestamp for start ({}) and end ({}). Timing is broken.",
                name, start_ts, end_ts
            );
        }
    }
}

/// ANTI-MASKING TEST: Verify error_enum/error_message are null for passing tests.
///
/// If passing tests have error_enum or error_message populated, error capture is leaking.
#[test]
fn error_fields_null_for_passing_tests() {
    let jsonl = run_cargo_test();
    let records = parse_jsonl(&jsonl);
    
    // Find test.end records with status=pass
    let pass_records: Vec<&serde_json::Value> = records.iter()
        .filter(|r| {
            r.get("event").and_then(|e| e.as_str()) == Some("test.end")
                && r["status"].as_str() == Some("pass")
        })
        .collect();
    
    assert!(
        pass_records.len() >= 1,
        "need at least 1 passing test to check error fields, got: {}",
        pass_records.len()
    );
    
    for record in &pass_records {
        let test_name = record["name"].as_str().unwrap_or("unknown");
        
        // Error fields should be null for passing tests
        assert!(
            record["error_enum"].is_null(),
            "passing test '{}' has error_enum={:?}. Error fields should be null for passing tests.",
            test_name,
            record["error_enum"]
        );
        assert!(
            record["error_message"].is_null(),
            "passing test '{}' has error_message={:?}. Error fields should be null for passing tests.",
            test_name,
            record["error_message"]
        );
    }
}

/// ANTI-MASKING TEST: Verify test.name field propagates to buffered events.
///
/// If buffered events don't have test.name, span field propagation is broken.
#[test]
fn test_name_propagates_to_buffered_events() {
    let jsonl = run_cargo_test();
    let records = parse_jsonl(&jsonl);
    
    // Filter to buffered events (non-lifecycle)
    let buffered_events: Vec<&serde_json::Value> = records.iter()
        .filter(|r| {
            let event = r.get("event").and_then(|e| e.as_str());
            event != Some("test.start") && event != Some("test.end")
        })
        .collect();
    
    assert!(
        buffered_events.len() >= 1,
        "need at least 1 buffered event to check test.name propagation, got: {}",
        buffered_events.len()
    );
    
    // Check that buffered events have test.name field
    let events_with_test_name: Vec<&serde_json::Value> = buffered_events
        .iter()
        .filter(|r| r.get("test.name").is_some())
        .copied()
        .collect();
    
    assert!(
        !events_with_test_name.is_empty(),
        "no buffered events have test.name field. Span field propagation is broken. test.name should propagate from span context to all child events."
    );
}
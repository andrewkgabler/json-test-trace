// Copyright (C) 2019-2024 Daniel Mueller <deso@posteo.net>
// SPDX-License-Identifier: (Apache-2.0 OR MIT)

//! Demonstrates different error patterns from allograph codebase.
//! Run with: cargo test --test error_patterns -- --nocapture

use std::fmt;

// --- Mimic allograph error enum ---

#[derive(Debug, Clone, PartialEq)]
pub enum AllographError {
    AllographDbQuery { message: String },
    AllographDbDdl { message: String },
    UserInput { message: String },
    AdminSdl { message: String },
}

impl fmt::Display for AllographError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AllographDbQuery { message } => write!(f, "{}", message),
            Self::AllographDbDdl { message } => write!(f, "{}", message),
            Self::UserInput { message } => write!(f, "{}", message),
            Self::AdminSdl { message } => write!(f, "{}", message),
        }
    }
}

impl std::error::Error for AllographError {}

// --- Pattern 1: Test assertion panic (assert_eq!) ---

#[json_test_trace::test]
fn pattern_assert_eq_panic() {
    tracing::info!("test start");
    assert_eq!(1, 2, "values should match");
    tracing::info!("never reached");
}

// --- Pattern 2: Test assertion panic (assert!) ---

#[json_test_trace::test]
fn pattern_assert_panic() {
    tracing::info!("test start");
    assert!(false, "condition must be true");
    tracing::info!("never reached");
}

// --- Pattern 3: Test assertion panic (unwrap on Err) ---

#[json_test_trace::test]
fn pattern_unwrap_err() {
    tracing::info!("test start");
    let result: Result<i32, AllographError> = Err(AllographError::UserInput {
        message: "missing required field".to_string(),
    });
    result.unwrap();
    tracing::info!("never reached");
}

// --- Pattern 4: panic! with error enum Display ---

#[json_test_trace::test]
fn pattern_panic_with_error_display() {
    tracing::info!("test start");
    let err = AllographError::AllographDbQuery {
        message: "unique constraint violation: email".to_string(),
    };
    panic!("{}", err);
}

// --- Pattern 5: panic! with error enum Debug ---

#[json_test_trace::test]
fn pattern_panic_with_error_debug() {
    tracing::info!("test start");
    let err = AllographError::UserInput {
        message: "duplicate @id value".to_string(),
    };
    panic!("{:?}", err);
}

// --- Pattern 6: tracing::error! (application error path) ---

#[json_test_trace::test]
fn pattern_tracing_error() {
    tracing::info!("test start");
    tracing::error!(error = ?AllographError::AllographDbQuery { message: "connection timeout".to_string() }, code = "allograph-db-query", party = ?"Allograph", "GraphQL error");
    tracing::info!("test continues after error");
}

// --- Pattern 7: Result-returning test with Err (no panic) ---

#[json_test_trace::test]
fn pattern_result_err_no_panic() -> Result<(), AllographError> {
    tracing::info!("test start");
    Err(AllographError::AdminSdl {
        message: "type conflict: User already exists".to_string(),
    })
}

// --- Pattern 8: Result-returning test Ok (success) ---

#[json_test_trace::test]
fn pattern_result_ok() -> Result<(), AllographError> {
    tracing::info!("test start");
    Ok(())
}

// --- Pattern 9: map_err + ? operator (common allograph pattern) ---

#[json_test_trace::test]
fn pattern_map_err_question_mark() -> Result<(), AllographError> {
    tracing::info!("test start");
    let result: Result<i32, AllographError> = Err(AllographError::AllographDbDdl {
        message: "relation does not exist".to_string(),
    });
    let _val = result.map_err(|e| {
        tracing::error!(error = ?e, code = "allograph-db-ddl", "DB error");
        e
    })?;
    Ok(())
}

// --- Pattern 10: unwrap_or_else panic (common in allograph tests) ---

#[json_test_trace::test]
fn pattern_unwrap_or_else_panic() {
    tracing::info!("test start");
    let data: Option<String> = None;
    let val = data.unwrap_or_else(|| panic!("queryDocument is not an array"));
    tracing::info!("never reached: {}", val);
}
// Copyright (C) 2019-2024 Daniel Mueller <deso@posteo.net>
// SPDX-License-Identifier: (Apache-2.0 OR MIT)

//! Demo: test helper that logs GraphQL errors to tracing before panicking.

use std::fmt;

// --- Mimic allograph error enum ---

#[derive(Debug, Clone, PartialEq)]
pub enum AllographError {
    AllographDbQuery { message: String },
    UserInput { message: String },
    AdminSdl { message: String },
}

impl fmt::Display for AllographError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AllographDbQuery { message } => write!(f, "{}", message),
            Self::UserInput { message } => write!(f, "{}", message),
            Self::AdminSdl { message } => write!(f, "{}", message),
        }
    }
}

impl AllographError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::AllographDbQuery { .. } => "allograph-db-query",
            Self::UserInput { .. } => "user-input",
            Self::AdminSdl { .. } => "admin-sdl",
        }
    }
}

// --- Mimic GraphQL response ---

#[derive(Debug, Clone)]
pub struct GraphqlResponse {
    pub data: Option<serde_json::Value>,
    pub errors: Vec<AllographError>,
}

// ============================================================
// OPTION 1: Helper function (manual call in tests)
// ============================================================

pub fn assert_no_graphql_errors(response: &GraphqlResponse) {
    if !response.errors.is_empty() {
        // Log EACH error to tracing (goes to JSONL)
        for err in &response.errors {
            tracing::error!(
                error = ?err,
                code = err.code(),
                "GraphQL error in response"
            );
        }
        // Then panic with Debug format
        panic!("GraphQL error: {:?}", response.errors);
    }
}

// ============================================================
// OPTION 2: Macro (drop-in replacement for assert!)
// ============================================================

#[macro_export]
macro_rules! assert_no_graphql_errors {
    ($response:expr) => {{
        let response = &$response;
        if !response.errors.is_empty() {
            for err in &response.errors {
                tracing::error!(
                    error = ?err,
                    code = err.code(),
                    "GraphQL error in response"
                );
            }
            panic!("GraphQL error: {:?}", response.errors);
        }
    }};
}

// ============================================================
// OPTION 3: Trait (method syntax, chainable)
// ============================================================

pub trait AssertNoGraphqlErrors {
    fn assert_no_errors(self);
}

impl AssertNoGraphqlErrors for GraphqlResponse {
    fn assert_no_errors(self) {
        if !self.errors.is_empty() {
            for err in &self.errors {
                tracing::error!(
                    error = ?err,
                    code = err.code(),
                    "GraphQL error in response"
                );
            }
            panic!("GraphQL error: {:?}", self.errors);
        }
    }
}

// ============================================================
// TESTS
// ============================================================

#[json_test_trace::test]
fn option1_helper_function() {
    let response = GraphqlResponse {
        data: None,
        errors: vec![AllographError::UserInput {
            message: "duplicate @id value: user-123".to_string(),
        }],
    };
    // Manual call
    assert_no_graphql_errors(&response);
}

#[json_test_trace::test]
fn option2_macro() {
    let response = GraphqlResponse {
        data: None,
        errors: vec![AllographError::AdminSdl {
            message: "type conflict: User already exists".to_string(),
        }],
    };
    // Drop-in replacement for assert!
    assert_no_graphql_errors!(response);
}

#[json_test_trace::test]
fn option3_trait() {
    let response = GraphqlResponse {
        data: None,
        errors: vec![AllographError::AllographDbQuery {
            message: "unique constraint violation: email".to_string(),
        }],
    };
    // Method syntax
    response.assert_no_errors();
}

#[json_test_trace::test]
fn option1_no_errors_passes() {
    let response = GraphqlResponse {
        data: Some(serde_json::json!({"user": {"name": "test"}})),
        errors: vec![],
    };
    assert_no_graphql_errors(&response);
}
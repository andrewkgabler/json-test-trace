// Copyright (C) 2019-2023 Daniel Mueller <deso@posteo.net>
// SPDX-License-Identifier: (Apache-2.0 OR MIT)

//! Global test logging subscriber that writes JSONL to a file.
//!
//! Each test's events are accumulated in a per-test buffer and flushed
//! atomically as a contiguous block on test completion.

use std::collections::HashMap;
use std::env;
use std::fmt;
use std::fs::File;
use std::io::{self, Write};
use std::sync::{Mutex, OnceLock};
use std::time::SystemTime;

use serde_json::{self, Value};

use tracing_subscriber::layer::Layer;
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::prelude::__tracing_subscriber_SubscriberExt;

/// Global state for the test logging subscriber.
#[derive(Debug)]
struct TestLogState {
    /// Path to the JSONL log file.
    log_file_path: String,
    /// The file handle (opened once, appended to).
    file: Mutex<File>,
    /// Per-test buffers keyed by test name.
    test_buffers: Mutex<HashMap<String, Vec<u8>>>,
    /// Current test name being tracked (set by macro via span).
    current_test: Mutex<Option<String>>,
}

impl TestLogState {
    fn new(log_file_path: String) -> io::Result<Self> {
        let file = File::create(&log_file_path)?;
        Ok(Self {
            log_file_path,
            file: Mutex::new(file),
            test_buffers: Mutex::new(HashMap::new()),
            current_test: Mutex::new(None),
        })
    }
}

/// Global subscriber state (initialized once per process).
pub static STATE: OnceLock<TestLogState> = OnceLock::new();

/// Get the log file path from the global state, if initialized.
pub fn get_log_file_path() -> Option<String> {
    STATE.get().map(|s| s.log_file_path.clone())
}

/// Clear the current test name in global state.
fn clear_current_test(state: &TestLogState) {
    if let Ok(mut current) = state.current_test.lock() {
        *current = None;
    }
}

/// Initialize the global test logging subscriber.
///
/// Reads `TEST_LOG_FILE` from environment, creates/truncates the file,
/// and installs a global subscriber that writes JSONL to the file.
pub fn init_global_test_logging() {
    // If already initialized, return early (idempotent for parallel tests)
    if STATE.get().is_some() {
        return;
    }

    let log_file_path = env::var("TEST_LOG_FILE").unwrap_or_else(|_| {
        // Default: /tmp/{crate_name}-tracing.jsonl
        format!("/tmp/json-test-trace.jsonl")
    });

    // Validate path: must be writable, no path traversal
    if let Some(parent) = std::path::Path::new(&log_file_path).parent() {
        if !parent.exists() {
            eprintln!("json-test-trace: log file parent directory does not exist: {}", parent.display());
            return;
        }
        if !parent.is_dir() {
            eprintln!("json-test-trace: log file parent is not a directory: {}", parent.display());
            return;
        }
    }

    let state = match TestLogState::new(log_file_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("json-test-trace: failed to create test log file: {}", e);
            return;
        }
    };

    // Use get_or_init to handle race between check and set
    if STATE.set(state).is_err() {
        // Another thread beat us to it — subscriber already installed
        return;
    }

    // Install the global subscriber
    let filter = tracing_subscriber::EnvFilter::builder()
        .with_default_directive(
            tracing_subscriber::filter::LevelFilter::TRACE.into()
        )
        .from_env_lossy();

    let layer = TestLogLayer;

    tracing_subscriber::registry()
        .with(filter)
        .with(layer)
        .init();
}

/// A custom Layer that writes tracing events to per-test buffers.
struct TestLogLayer;

impl<S> Layer<S> for TestLogLayer
where
    S: for<'a> LookupSpan<'a> + tracing::Subscriber,
{
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        // Use current_test set by on_new_span when the test span was created.
        // The test span has test.name field; child events inherit the span context.
        let test_name = if let Some(state) = STATE.get() {
            let current = state.current_test.lock().ok();
            current.and_then(|c| c.clone())
        } else {
            None
        };
        
        if let Some(name) = test_name {
            // Serialize event to JSON
            if let Some(state) = STATE.get() {
                let json = serialize_event(event);
                if let Some(json) = json {
                    let mut buffers = state.test_buffers.lock().ok();
                    if let Some(ref mut buffers) = buffers {
                        let buffer = buffers.entry(name).or_insert_with(Vec::new);
                        buffer.extend_from_slice(json.as_bytes());
                        buffer.push(b'\n');
                    }
                }
            }
        }
    }

    fn on_new_span(
        &self,
        attrs: &tracing::span::Attributes<'_>,
        _id: &tracing::span::Id,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        // Track test name from span fields
        if let Some(test_name) = extract_test_name_from_span(attrs) {
            eprintln!("json-test-trace: on_new_span test.name={}", test_name);
            if let Some(state) = STATE.get() {
                if let Ok(mut current) = state.current_test.lock() {
                    *current = Some(test_name);
                }
            }
        }
    }

fn on_close(
        &self,
        _id: tracing::span::Id,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        // Clear current test name on span close.
        // Do NOT flush here — TestGuard::drop() handles flushing.
        if let Some(state) = STATE.get() {
            clear_current_test(state);
        }
    }
}

/// Visitor to extract event fields for JSON serialization.
struct EventVisitor {
    timestamp: u64,
    level: String,
    target: String,
    fields: HashMap<String, Value>,
}

impl EventVisitor {
    fn new() -> Self {
        Self {
            timestamp: 0,
            level: "INFO".to_string(),
            target: "unknown".to_string(),
            fields: HashMap::new(),
        }
    }

    fn parse_value(s: &str) -> Value {
        // Try to parse as number
        if let Ok(n) = s.parse::<i64>() {
            return Value::Number(n.into());
        }
        if let Ok(n) = s.parse::<f64>() {
            if let Some(num) = serde_json::Number::from_f64(n) {
                return Value::Number(num);
            }
        }
        // Try to parse as bool
        match s {
            "true" => return Value::Bool(true),
            "false" => return Value::Bool(false),
            "null" => return Value::Null,
            _ => {}
        }
        // Fallback to string
        Value::String(s.to_string())
    }
}

impl tracing::field::Visit for EventVisitor {
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        match field.name() {
            "timestamp" => {
                if let Ok(ns) = value.parse::<u64>() {
                    self.timestamp = ns;
                }
            }
            "level" => self.level = value.to_string(),
            "target" => self.target = value.to_string(),
            _ => {
                self.fields.insert(field.name().to_string(), Self::parse_value(value));
            }
        }
    }

    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn fmt::Debug) {
        match field.name() {
            "timestamp" => {
                let debug_str = format!("{:?}", value);
                if let Ok(ns) = debug_str.parse::<u64>() {
                    self.timestamp = ns;
                }
            }
            _ => {
                let debug_str = format!("{:?}", value);
                self.fields.insert(field.name().to_string(), Self::parse_value(&debug_str));
            }
        }
    }
}

/// Serialize a tracing event to JSON.
fn serialize_event(event: &tracing::Event<'_>) -> Option<String> {
    let mut visitor = EventVisitor::new();
    event.record(&mut visitor);
    
    let timestamp = if visitor.timestamp == 0 {
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64
    } else {
        visitor.timestamp
    };
    
    let level = visitor.level;
    let target = visitor.target;
    let fields = visitor.fields;
    
    // Build JSON object using serde_json::Value
    let mut map = serde_json::Map::new();
    map.insert("timestamp".to_string(), Value::Number(
        serde_json::Number::from(timestamp)
    ));
    map.insert("level".to_string(), Value::String(level));
    map.insert("fields".to_string(), Value::Object(
        fields.into_iter().collect()
    ));
    map.insert("target".to_string(), Value::String(target));
    
    let value = Value::Object(map);
    serde_json::to_string(&value).ok()
}

/// Visitor to extract test.name field.
struct TestNameVisitor {
    name: Option<String>,
}

impl TestNameVisitor {
    fn new() -> Self {
        Self { name: None }
    }
}

impl tracing::field::Visit for TestNameVisitor {
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "test.name" {
            self.name = Some(value.to_string());
        }
    }

    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn fmt::Debug) {
        if field.name() == "test.name" {
            self.name = Some(format!("{:?}", value));
        }
    }
}

/// Extract test name from an event's span context.
fn extract_test_name(event: &tracing::Event<'_>) -> Option<String> {
    let mut visitor = TestNameVisitor::new();
    event.record(&mut visitor);
    visitor.name
}

/// Extract test name from span attributes.
fn extract_test_name_from_span(attrs: &tracing::span::Attributes<'_>) -> Option<String> {
    let mut visitor = TestNameVisitor::new();
    attrs.record(&mut visitor);
    visitor.name
}

/// Guard that flushes test logs on drop.
///
/// Created by the macro at the start of each test, dropped at the end
/// (including on panic). Flushes all accumulated JSONL for the test.
pub struct TestGuard {
    test_name: String,
    test_module: String,
}

impl TestGuard {
    /// Create a new TestGuard for the given test.
    pub fn new(test_name: &str, test_module: &str) -> Self {
        if let Some(state) = STATE.get() {
            // Clear any existing buffer for this test
            if let Ok(mut buffers) = state.test_buffers.lock() {
                buffers.remove(test_name);
            }
            // Set current test
            if let Ok(mut current) = state.current_test.lock() {
                *current = Some(test_name.to_string());
            }
        }
        Self {
            test_name: test_name.to_string(),
            test_module: test_module.to_string(),
        }
    }

    /// Flush the test's JSONL buffer to the log file.
    fn flush(&self) {
        if let Some(state) = STATE.get() {
            if let Ok(mut buffers) = state.test_buffers.lock() {
                if let Some(buffer) = buffers.remove(&self.test_name) {
                    if let Ok(mut file) = state.file.lock() {
                        // Emit synthetic test.start record
                        let start_ts = SystemTime::now()
                            .duration_since(SystemTime::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_nanos();
                        let start_record = format!(
                            r#"{{"event":"test.start","name":"{}","module":"{}","status":"pass","error":null,"fields":{{"test.name":"{}","test.module":"{}","status":"pass"}}, "level":"INFO","target":"{}","timestamp":{}}}"#,
                            self.test_name,
                            self.test_module,
                            self.test_name,
                            self.test_module,
                            self.test_module,
                            start_ts
                        );
                        let _ = file.write_all(start_record.as_bytes());
                        let _ = file.write_all(b"\n");
                        
                        // Write buffered events
                        let _ = file.write_all(&buffer);
                        
                        // Emit synthetic test.end record
                        let end_ts = SystemTime::now()
                            .duration_since(SystemTime::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_nanos();
                        let end_record = format!(
                            r#"{{"event":"test.end","name":"{}","module":"{}","duration_ms":0.0,"fields":{{"test.name":"{}","test.module":"{}","status":"pass"}}, "level":"INFO","target":"{}","timestamp":{}}}"#,
                            self.test_name,
                            self.test_module,
                            self.test_name,
                            self.test_module,
                            self.test_module,
                            end_ts
                        );
                        let _ = file.write_all(end_record.as_bytes());
                        let _ = file.write_all(b"\n");
                        
                        let _ = file.flush();
                    }
                }
            }
            clear_current_test(state);
        }
    }
}

impl Drop for TestGuard {
    fn drop(&mut self) {
        self.flush();
    }
}

/// Initialize global logging and create a TestGuard for the current test.
///
/// This is called by the macro at the start of each test.
pub fn init_test(test_name: &str, test_module: &str) -> TestGuard {
    // Initialize global state on first call
    init_global_test_logging();
    TestGuard::new(test_name, test_module)
}
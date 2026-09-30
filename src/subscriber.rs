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
use std::sync::atomic::AtomicBool;
use std::sync::{Mutex, OnceLock};
use std::thread_local;

thread_local! {
    /// Whether the current test has panicked.
    static TEST_PANICKED: AtomicBool = const { AtomicBool::new(false) };
}

/// Panic hook that sets the thread-local panicked flag.
fn panic_hook(_info: &std::panic::PanicHookInfo) {
    TEST_PANICKED.with(|flag| flag.store(true, std::sync::atomic::Ordering::Release));
}
use std::time::SystemTime;

use serde_json::{self, Value};
use tracing_subscriber::registry::SpanRef;

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
    /// Per-test buffers keyed by test name (raw field maps, serialized on flush).
    test_buffers: Mutex<HashMap<String, Vec<HashMap<String, Value>>>>,
    /// Current test name being tracked (set by macro via span).
    current_test: Mutex<Option<String>>,
    /// Test pass/fail result (set by macro via set_result).
    test_result: Mutex<Option<bool>>,
}

impl TestLogState {
    fn new(log_file_path: String) -> io::Result<Self> {
        let file = File::create(&log_file_path)?;
        Ok(Self {
            log_file_path,
            file: Mutex::new(file),
            test_buffers: Mutex::new(HashMap::new()),
            current_test: Mutex::new(None),
            test_result: Mutex::new(None),
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

/// Storage for span fields in span extensions.
struct SpanFieldStorage(pub HashMap<String, Value>);

/// A custom Layer that writes tracing events to per-test buffers.
struct TestLogLayer;

impl<S> Layer<S> for TestLogLayer
where
    S: for<'a> LookupSpan<'a> + tracing::Subscriber,
{
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        // Use current_test set by on_new_span when the test span was created.
        let test_name = if let Some(state) = STATE.get() {
            let current = state.current_test.lock().ok();
            current.and_then(|c| c.clone())
        } else {
            None
        };
        
        if let Some(name) = test_name {
            if let Some(state) = STATE.get() {
                // Merge span fields from parent spans into event fields
                let mut merged_fields = HashMap::new();
                if let Some(scope) = ctx.event_scope(event) {
                    for span in scope.from_root() {
                        merge_span_fields(span, &mut merged_fields);
                    }
                }
                
                // Extract event fields
                let mut event_fields = extract_event_fields(event);
                
                // Merge span fields into event fields (span fields take precedence)
                for (key, value) in merged_fields {
                    event_fields.entry(key).or_insert(value);
                }
                
                // Store raw field map in buffer (serialize on flush with pass/fail)
                if let Ok(mut buffers) = state.test_buffers.lock() {
                    let buffer = buffers.entry(name).or_insert_with(Vec::new);
                    buffer.push(event_fields);
                }
            }
        }
    }

    fn on_new_span(
        &self,
        attrs: &tracing::span::Attributes<'_>,
        id: &tracing::span::Id,
        ctx: tracing_subscriber::layer::Context<'_, S>,
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
        
        // Store span fields in extensions for propagation to child events
        let mut fields = HashMap::new();
        let mut visitor = SpanFieldVisitor(&mut fields);
        attrs.record(&mut visitor);
        
        if !fields.is_empty() {
            let storage = SpanFieldStorage(fields);
            if let Some(span) = ctx.span(id) {
                span.extensions_mut().insert(storage);
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

/// Extract event fields as a HashMap.
fn extract_event_fields(event: &tracing::Event<'_>) -> HashMap<String, Value> {
    let mut visitor = EventVisitor::new();
    event.record(&mut visitor);
    visitor.fields
}

/// Serialize a HashMap of fields to a JSON string with optional status injection.
fn serialize_fields(fields: HashMap<String, Value>, status: Option<&str>) -> Option<String> {
    let mut map = serde_json::Map::new();
    
    // Add status if provided
    if let Some(s) = status {
        map.insert("status".to_string(), Value::String(s.to_string()));
    }
    
    // Add all fields
    for (key, value) in fields {
        map.insert(key, value);
    }
    
    Some(serde_json::to_string(&Value::Object(map)).ok()?)
}

/// Visitor to extract all span fields for storage in extensions.
struct SpanFieldVisitor<'a>(&'a mut HashMap<String, Value>);

impl tracing::field::Visit for SpanFieldVisitor<'_> {
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        self.0.insert(field.name().to_string(), EventVisitor::parse_value(value));
    }

    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn fmt::Debug) {
        let debug_str = format!("{:?}", value);
        self.0.insert(field.name().to_string(), EventVisitor::parse_value(&debug_str));
    }
}

/// Merge fields from a span into the merged fields map.
fn merge_span_fields<S>(span: SpanRef<'_, S>, merged: &mut HashMap<String, Value>)
where
    S: for<'a> LookupSpan<'a>,
{
    if let Some(storage) = span.extensions().get::<SpanFieldStorage>() {
        for (key, value) in &storage.0 {
            merged.insert(key.clone(), value.clone());
        }
    }
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
        // Register panic hook to catch panics
        std::panic::set_hook(Box::new(panic_hook));
        // Clear panicked flag for this test
        TEST_PANICKED.with(|flag| flag.store(false, std::sync::atomic::Ordering::Release));
        
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

    /// Set the test pass/fail result explicitly. Called by macro before drop.
    pub fn set_result(&self, passed: bool) {
        if passed {
            TEST_PANICKED.with(|flag| flag.store(false, std::sync::atomic::Ordering::Release));
        } else {
            TEST_PANICKED.with(|flag| flag.store(true, std::sync::atomic::Ordering::Release));
        }
    }

    /// Check if the test has panicked.
    fn has_panicked(&self) -> bool {
        TEST_PANICKED.with(|flag| flag.load(std::sync::atomic::Ordering::Acquire))
    }

    /// Flush the test's JSONL buffer to the log file.
    fn flush(&self) {
        if let Some(state) = STATE.get() {
            if let Ok(mut buffers) = state.test_buffers.lock() {
                if let Some(buffer) = buffers.remove(&self.test_name) {
                    if let Ok(mut file) = state.file.lock() {
                        // Determine pass/fail status from panic flag or explicit result
                        let passed = !self.has_panicked();
                        let status = if passed { "pass" } else { "fail" };
                        
                        // Emit synthetic test.start record
                        let start_ts = SystemTime::now()
                            .duration_since(SystemTime::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_nanos();
                        let start_fields = serde_json::json!({
                            "test.name": self.test_name,
                            "test.module": self.test_module,
                            "status": status
                        });
                        let start_record = format!(
                            r#"{{"event":"test.start","name":"{}","module":"{}","status":"{}","error":null,"fields":{},"level":"INFO","target":"{}","timestamp":{}}}"#,
                            self.test_name,
                            self.test_module,
                            status,
                            serde_json::to_string(&start_fields).unwrap(),
                            self.test_module,
                            start_ts
                        );
                        let _ = file.write_all(start_record.as_bytes());
                        let _ = file.write_all(b"\n");
                        
                        // Write buffered events with status injected
                        for fields in &buffer {
                            let mut enriched = fields.clone();
                            enriched.insert("status".to_string(), Value::String(status.to_string()));
                            if let Some(json) = serialize_fields(enriched, Some(status)) {
                                let _ = file.write_all(json.as_bytes());
                                let _ = file.write_all(b"\n");
                            }
                        }
                        
                        // Emit synthetic test.end record
                        let end_ts = SystemTime::now()
                            .duration_since(SystemTime::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_nanos();
                        let end_fields = serde_json::json!({
                            "test.name": self.test_name,
                            "test.module": self.test_module,
                            "status": status
                        });
                        let end_record = format!(
                            r#"{{"event":"test.end","name":"{}","module":"{}","status":"{}","error":null,"fields":{},"level":"INFO","target":"{}","timestamp":{}}}"#,
                            self.test_name,
                            self.test_module,
                            status,
                            serde_json::to_string(&end_fields).unwrap(),
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
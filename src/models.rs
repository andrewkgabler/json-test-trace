use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::collections::HashMap;

fn deserialize_timestamp<'de, D>(deserializer: D) -> Result<Option<DateTime<Utc>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error;
    use serde_json::Value;

    let value = Value::deserialize(deserializer)?;
    match value {
        Value::Null => Ok(None),
        Value::String(s) => {
            // RFC 3339 string
            DateTime::parse_from_rfc3339(&s)
                .map(|dt| Some(dt.with_timezone(&Utc)))
                .map_err(|e| D::Error::custom(format!("invalid RFC 3339 timestamp: {}", e)))
        }
        Value::Number(n) => {
            // Nanosecond integer
            n.as_u64()
                .map(|ns| {
                    let secs = ns / 1_000_000_000;
                    let nanos = (ns % 1_000_000_000) as u32;
                    Some(DateTime::<Utc>::from_timestamp(secs as i64, nanos).unwrap_or_else(chrono::Utc::now))
                })
                .ok_or_else(|| D::Error::custom("invalid nanosecond timestamp"))
        }
        _ => Ok(None),
    }
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct JsonLogLine {
    #[serde(default, deserialize_with = "deserialize_timestamp")]
    pub timestamp: Option<DateTime<Utc>>,
    #[serde(default)]
    pub level: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub fields: Option<Fields>,
    #[serde(default)]
    pub target: Option<String>,
    #[serde(default, rename = "test.name")]
    pub test_name: Option<String>,
    #[serde(default, rename = "test.module")]
    pub test_module: Option<String>,
    #[serde(default)]
    pub span: Option<SpanInfo>,
    #[serde(default)]
    pub spans: Option<Vec<SpanInfo>>,
    #[serde(default, rename = "threadId", alias = "thread_id")]
    pub thread_id: Option<String>,
    #[serde(default)]
    pub event: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub module: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub error_enum: Option<String>,
    #[serde(default)]
    pub error_message: Option<String>,
    #[serde(default)]
    pub duration_ms: Option<f64>,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct Fields {
    pub message: Option<String>,
    #[serde(default, rename = "test.name")]
    pub test_name: Option<String>,
    #[serde(default, rename = "test.module")]
    pub test_module: Option<String>,
    #[serde(default, rename = "time.busy")]
    pub time_busy: Option<String>,
    #[serde(default, rename = "time.idle")]
    pub time_idle: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default, rename = "payload")]
    pub payload: Option<String>,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct SpanInfo {
    #[serde(default)]
    pub name: Option<String>,
}

#[derive(Debug, Clone)]
pub enum LogEvent {
    SpanEvent {
        message: String,
        span_name: String,
        target: String,
        level: String,
        timestamp: DateTime<Utc>,
        time_busy: Option<String>,
        time_idle: Option<String>,
        error: Option<String>,
        error_enum: Option<String>,
        error_message: Option<String>,
    },
}

#[derive(Debug, Clone)]
pub struct TestResult {
    pub name: String,
    pub module: String,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub duration_ms: Option<f64>,
    pub panicked: bool,
    pub panic_message: Option<String>,
    pub has_errors: bool,
    pub ignored: bool,
    pub log_events: Vec<LogEvent>,
    pub status: String,
    pub error: Option<String>,
}

impl TestResult {
    pub fn is_passed(&self) -> bool {
        self.status == "pass" && !self.has_errors && !self.panicked && !self.ignored
    }
}

pub struct TestParser {
    current_tests: HashMap<String, TestResult>,
    completed: Vec<TestResult>,
}

impl TestParser {
    pub fn new() -> Self {
        Self {
            current_tests: HashMap::new(),
            completed: Vec::new(),
        }
    }

    pub fn parse_line(&mut self, line: &str) -> Result<Option<TestResult>, serde_json::Error> {
        let log_line: JsonLogLine = serde_json::from_str(line)?;
        Ok(self.process_log_line(log_line))
    }

    fn process_log_line(&mut self, log: JsonLogLine) -> Option<TestResult> {
        // Handle synthetic test.start event
        if let Some(ref event) = log.event {
            if event == "test.start" {
                if let (Some(name), Some(module)) = (&log.name, &log.module) {
                    self.current_tests.insert(name.clone(), TestResult {
                        name: name.clone(),
                        module: module.clone(),
                        started_at: log.timestamp.unwrap_or_else(chrono::Utc::now),
                        ended_at: None,
                        duration_ms: None,
                        panicked: false,
                        panic_message: None,
                        has_errors: false,
                        ignored: false,
                        log_events: Vec::new(),
                        status: log.status.clone().unwrap_or_else(|| "pass".to_string()),
                        error: log.error.clone(),
                    });
                }
                return None;
            }
            
            // Handle synthetic test.end event
            if event == "test.end" {
                if let Some(name) = &log.name {
                    if let Some(mut test) = self.current_tests.remove(name) {
                        if let Some(duration) = log.duration_ms {
                            test.duration_ms = Some(duration);
                        }
                        self.completed.push(test);
                        return None;
                    }
                }
                return None;
            }
            
            // Handle test.ignored event
            if event == "test.ignored" {
                if let Some(name) = &log.name {
                    if let Some(test) = self.current_tests.get_mut(name) {
                        test.ignored = true;
                    }
                }
                return None;
            }
        }
        
        // Handle regular tracing events (span events)
        if let Some(ref message) = log.message.or_else(|| log.fields.as_ref().and_then(|f| f.message.clone())) {
            // Find the test that this event belongs to by checking span.name, target, or test.name
            let test_key = log.test_name.clone()
                .or_else(|| log.fields.as_ref().and_then(|f| f.test_name.clone()))
                .or_else(|| log.target.clone());
            
            if let Some(ref key) = test_key {
                for test in self.current_tests.values_mut() {
                    if test.module == *key || test.name == *key {
                        if message == "panic" {
                            test.panicked = true;
                            test.panic_message = log.fields.as_ref().and_then(|f| f.payload.clone());
                        }
                        if log.fields.as_ref().map(|f| f.error.is_some()).unwrap_or(false) {
                            test.has_errors = true;
                        }
                        
                        let span_name = log.span.as_ref().and_then(|s| s.name.clone()).unwrap_or_default();
                        
                        test.log_events.push(LogEvent::SpanEvent {
                            message: message.clone(),
                            span_name,
                            target: log.target.clone().unwrap_or_default(),
                            level: log.level.clone().unwrap_or_default(),
                            timestamp: log.timestamp.unwrap_or(chrono::Utc::now()),
                            time_busy: log.fields.as_ref().and_then(|f| f.time_busy.clone()),
                            time_idle: log.fields.as_ref().and_then(|f| f.time_idle.clone()),
                            error: log.fields.as_ref().and_then(|f| f.error.clone()),
                            error_enum: log.error_enum.clone(),
                            error_message: log.error_message.clone(),
                        });
                        break;
                    }
                }
            }
        }
        
        None
    }

    pub fn finalize(&mut self) -> Vec<TestResult> {
        let mut results: Vec<TestResult> = self.completed.drain(..).collect();
        for (_, test) in self.current_tests.drain() {
            results.push(test);
        }
        Self::build_sorted(results)
    }

    pub fn results(&self) -> Vec<TestResult> {
        let mut results: Vec<TestResult> = self.completed.iter().cloned().collect();
        for test in self.current_tests.values() {
            results.push(test.clone());
        }
        Self::build_sorted(results)
    }

    fn build_sorted(mut results: Vec<TestResult>) -> Vec<TestResult> {
        results.sort_by(|a, b| a.started_at.cmp(&b.started_at));
        results
    }
}

pub fn build_summary(results: Vec<TestResult>) -> serde_json::Value {
    let total = results.len();
    let passed = results.iter().filter(|t| t.is_passed()).count();
    let failed = total - passed;
    let ignored = results.iter().filter(|t| t.ignored).count();
    let panics = results.iter().filter(|t| t.panicked).count();
    
    // Group errors by error_enum, collecting messages
    let mut errors_by_enum: HashMap<String, Vec<String>> = HashMap::new();
    for test in &results {
        for event in &test.log_events {
            if let LogEvent::SpanEvent { error_enum: Some(enum_), error_message: Some(msg), .. } = event {
                errors_by_enum
                    .entry(enum_.clone())
                    .or_default()
                    .push(msg.clone());
            }
        }
    }
    
    // Also extract from error field for backward compatibility
    for test in &results {
        for event in &test.log_events {
            if let LogEvent::SpanEvent { error: Some(err), error_enum: None, .. } = event {
                if let Some((cat, _)) = extract_error_type(err) {
                    // Only add if not already present via error_enum
                    if !errors_by_enum.contains_key(cat) {
                        errors_by_enum.insert(cat.to_string(), Vec::new());
                    }
                }
            }
        }
    }
    
    let modules: Vec<String> = results.iter().map(|t| t.module.clone()).collect::<std::collections::HashSet<_>>().into_iter().collect();
    
    serde_json::json!({
        "tests": {
            "total": total,
            "passed": passed,
            "failed": failed,
            "ignored": ignored,
            "panicked": panics,
        },
        "errors": errors_by_enum.into_iter().map(|(enum_, messages)| serde_json::json!({
            "error_enum": enum_,
            "count": messages.len(),
            "messages": messages,
        })).collect::<Vec<_>>(),
        "modules": modules,
    })
}

pub fn extract_error_type(err: &str) -> Option<(&str, &str)> {
    // Extract error category from Debug format error string
    // e.g., "ValidationError { message: \"missing required field\" }" -> ("ValidationError", ...)
    let err = err.trim();
    let brace_pos = err.find('{')?;
    let category = err[..brace_pos].trim();
    Some((category, err))
}
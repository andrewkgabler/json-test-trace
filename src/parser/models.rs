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
            // Nanosecond integer since epoch
            n.as_u64()
                .map(|ns| Some(DateTime::<Utc>::from_timestamp_nanos(ns as i64)))
                .ok_or_else(|| D::Error::custom("timestamp number too large"))
        }
        _ => Err(D::Error::custom("timestamp must be a string or integer")),
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct JsonLogLine {
    #[serde(default, deserialize_with = "deserialize_timestamp")]
    pub timestamp: Option<DateTime<Utc>>,
    #[serde(default)]
    pub level: Option<String>,
    #[serde(default)]
    pub fields: Option<Fields>,
    #[serde(default)]
    pub target: Option<String>,
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
        timestamp: DateTime<chrono::Utc>,
        time_busy: Option<String>,
        time_idle: Option<String>,
        error: Option<String>,
    },
}

#[derive(Debug, Clone)]
pub struct TestResult {
    pub name: String,
    pub module: String,
    pub started_at: DateTime<chrono::Utc>,
    pub ended_at: Option<DateTime<chrono::Utc>>,
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
        self.ended_at.is_some() && !self.panicked && !self.ignored && self.status == "pass"
    }

    pub fn status_str(&self) -> &'static str {
        if self.ignored {
            "IGNORED"
        } else if self.panicked {
            "FAIL (panic)"
        } else if self.has_errors {
            "FAIL"
        } else if self.is_passed() {
            "PASS"
        } else {
            "FAIL"
        }
    }

    pub fn duration_str(&self) -> String {
        self.duration_ms
            .map(|d| format!("{:.1}ms", d))
            .unwrap_or_else(|| "N/A".to_string())
    }
}

pub struct TestParser {
    current_tests: HashMap<String, TestResult>,
    completed: Vec<TestResult>,
    last_active_test: Option<String>,
}

impl TestParser {
    pub fn new() -> Self {
        Self {
            current_tests: HashMap::new(),
            completed: Vec::new(),
            last_active_test: None,
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
                    self.last_active_test = Some(name.clone());
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
                        test.ended_at = Some(chrono::Utc::now());
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
        if let (Some(fields), Some(target)) = (&log.fields, &log.target) {
            if let Some(ref message) = fields.message {
                // Track last active test for this target
                for test in self.current_tests.values_mut() {
                    if test.module == *target || test.name == *target {
                        self.last_active_test = Some(test.name.clone());
                        break;
                    }
                }
                
                // Only attribute panic to the last active test
                if message == "panic" {
                    if let Some(ref active_name) = self.last_active_test {
                        if let Some(test) = self.current_tests.get_mut(active_name) {
                            test.panicked = true;
                            test.panic_message = fields.payload.clone();
                        }
                    }
                }
                
                // All tests matching this target get error flag
                for test in self.current_tests.values_mut() {
                    if test.module == *target || test.name == *target {
                        if fields.error.is_some() {
                            test.has_errors = true;
                        }
                        
                        let span_name = log.span.as_ref().map(|s| s.name.clone().unwrap_or_default()).unwrap_or_default();
                        
                        test.log_events.push(LogEvent::SpanEvent {
                            message: message.clone(),
                            span_name,
                            target: target.clone(),
                            level: log.level.clone().unwrap_or_default(),
                            timestamp: log.timestamp.unwrap_or(chrono::Utc::now()),
                            time_busy: fields.time_busy.clone(),
                            time_idle: fields.time_idle.clone(),
                            error: fields.error.clone(),
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

/// Extract error type (variant name) and message from a Debug-formatted error string.
///
/// Expected format: `VariantName { message: "..." }`
/// Returns `(category, message)` or `None` if the format doesn't match.
pub fn extract_error_type(error_str: &str) -> Option<(&str, &str)> {
    let trimmed = error_str.trim();
    let brace_pos = trimmed.find('{')?;
    let category = trimmed[..brace_pos].trim();
    if category.is_empty() || !category.chars().next().map_or(false, |c| c.is_ascii_alphabetic()) {
        return None;
    }
    let inner = trimmed[brace_pos + 1..].trim_end_matches('}').trim();
    let msg_prefix = "message: \"";
    let msg_start = inner.find(msg_prefix)? + msg_prefix.len();
    let rest = &inner[msg_start..];
    let mut end = 0;
    let bytes = rest.as_bytes();
    while end < bytes.len() {
        if bytes[end] == b'\\' && end + 1 < bytes.len() {
            match bytes[end + 1] {
                b'"' | b'\\' | b'n' | b'r' | b't' => end += 2,
                _ => end += 1,
            }
        } else if bytes[end] == b'"' {
            break;
        } else {
            end += 1;
        }
    }
    if end >= bytes.len() {
        return None;
    }
    Some((category, &rest[..end]))
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ModuleTree {
    pub module: String,
    pub count: usize,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub child: Vec<ModuleTree>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub test: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl ModuleTree {
    pub fn new(module: &str) -> Self {
        Self {
            module: module.to_string(),
            count: 0,
            child: Vec::new(),
            test: None,
            message: None,
        }
    }

    pub fn insert_test(&mut self, test_name: &str, message: &str) {
        self.count += 1;
        self.test = Some(test_name.to_string());
        self.message = Some(message.to_string());
    }

    pub fn find_or_create_child_idx(&mut self, module: &str) -> usize {
        if let Some(idx) = self.child.iter().position(|c| c.module == module) {
            idx
        } else {
            self.child.push(ModuleTree::new(module));
            self.child.len() - 1
        }
    }
}

pub fn build_module_tree(module_path: &str, test_name: &str, message: &str) -> ModuleTree {
    let trimmed = module_path.trim();
    let segments: Vec<&str> = if trimmed.is_empty() {
        vec!["unknown"]
    } else {
        trimmed.split("::").collect()
    };
    let mut root = ModuleTree::new(segments[0]);
    insert_at_depth(&mut root, &segments[1..], test_name, message);
    propagate_counts(&mut root);
    root
}

fn insert_at_depth(node: &mut ModuleTree, segments: &[&str], test_name: &str, message: &str) {
    if segments.is_empty() {
        node.insert_test(test_name, message);
        return;
    }
    let idx = node.find_or_create_child_idx(segments[0]);
    insert_at_depth(&mut node.child[idx], &segments[1..], test_name, message);
}

fn propagate_counts(node: &mut ModuleTree) {
    if node.child.is_empty() {
        return;
    }
    for child in &mut node.child {
        propagate_counts(child);
    }
    node.count = node.child.iter().map(|c| c.count).sum();
}

#[derive(Debug, serde::Serialize)]
pub struct ErrorGroup {
    pub category: String,
    pub tests: ModuleTree,
}

#[derive(Debug, serde::Serialize)]
pub struct TestCounts {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub ignored: usize,
}

#[derive(Debug, serde::Serialize)]
pub struct SummaryJson {
    pub errors: Vec<ErrorGroup>,
    pub tests: TestCounts,
}

pub fn build_summary(tests: &[TestResult]) -> SummaryJson {
    let total = tests.len();
    let (passed, ignored, failed) = tests.iter().fold((0usize, 0usize, 0usize), |(p, i, f), t| {
        if t.ignored {
            (p, i + 1, f)
        } else if t.is_passed() {
            (p + 1, i, f)
        } else {
            (p, i, f + 1)
        }
    });

    let mut error_map: std::collections::BTreeMap<String, Vec<(String, String, String)>> = std::collections::BTreeMap::new();
    let mut seen: std::collections::HashSet<(String, String)> = std::collections::HashSet::new();

    for test in tests.iter().filter(|t| (!t.is_passed() || t.has_errors) && !t.ignored) {
        for event in &test.log_events {
            if let LogEvent::SpanEvent { error: Some(err), .. } = event {
                if let Some((category, message)) = extract_error_type(err) {
                    let key = (category.to_string(), test.name.clone());
                    if seen.insert(key) {
                        error_map
                            .entry(category.to_string())
                            .or_default()
                            .push((test.module.clone(), test.name.clone(), message.to_string()));
                    }
                }
            }
        }
        if test.panicked {
            let key = ("Panic".to_string(), test.name.clone());
            if seen.insert(key) {
                let msg = test.panic_message.clone().unwrap_or_default();
                error_map
                    .entry("Panic".to_string())
                    .or_default()
                    .push((test.module.clone(), test.name.clone(), msg));
            }
        }
    }

    let errors: Vec<ErrorGroup> = error_map
        .into_iter()
        .map(|(category, entries)| {
            let mut tree: Option<ModuleTree> = None;
            for (module, test_name, message) in entries {
                let subtree = build_module_tree(&module, &test_name, &message);
                tree = Some(match tree {
                    None => subtree,
                    Some(mut existing) => {
                        merge_trees(&mut existing, subtree);
                        existing
                    }
                });
            }
            ErrorGroup { category, tests: tree.unwrap_or_else(|| ModuleTree::new("unknown")) }
        })
        .collect();

    SummaryJson {
        errors,
        tests: TestCounts { total, passed, failed, ignored },
    }
}

fn merge_trees(parent: &mut ModuleTree, mut child: ModuleTree) {
    if child.child.is_empty() {
        if let (Some(test), Some(msg)) = (child.test.take(), child.message.take()) {
            parent.insert_test(&test, &msg);
        }
        return;
    }
    for grandchild in child.child.drain(..) {
        let idx = parent.find_or_create_child_idx(&grandchild.module);
        let subtree = grandchild;
        let mut wrapper = ModuleTree::new(&subtree.module);
        wrapper.child = subtree.child;
        wrapper.count = subtree.count;
        wrapper.test = subtree.test;
        wrapper.message = subtree.message;
        merge_trees(&mut parent.child[idx], wrapper);
    }
    propagate_counts(parent);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test(name: &str, module: &str, passed: bool) -> TestResult {
        let now = chrono::Utc::now();
        TestResult {
            name: name.into(),
            module: module.into(),
            started_at: now,
            ended_at: Some(now),
            duration_ms: Some(10.0),
            panicked: !passed,
            panic_message: if passed { None } else { Some("assertion failed".into()) },
            has_errors: false,
            ignored: false,
            log_events: vec![],
            status: if passed { "pass".to_string() } else { "fail".to_string() },
            error: None,
        }
    }

    #[test]
    fn test_build_summary_no_errors() {
        let tests = vec![make_test("test_a", "allograph::auth", true)];
        let summary = build_summary(&tests);
        assert!(summary.errors.is_empty());
        assert_eq!(summary.tests.total, 1);
        assert_eq!(summary.tests.passed, 1);
        assert_eq!(summary.tests.failed, 0);
    }

    #[test]
    fn test_build_summary_with_error() {
        let mut t = make_test("test_fail", "allograph::auth::claims", false);
        t.panicked = false;
        t.panic_message = None;
        t.has_errors = true;
        t.status = "fail".to_string();
        t.error = Some("AllographDbQuery".to_string());
        t.log_events = vec![LogEvent::SpanEvent {
            message: "DB error".into(),
            span_name: "test_fail".into(),
            target: "allograph::auth::claims".into(),
            level: "ERROR".into(),
            timestamp: chrono::Utc::now(),
            time_busy: None,
            time_idle: None,
            error: Some("AllographDbQuery { message: \"unique constraint violation\" }".into()),
        }];
        let tests = vec![t];
        let summary = build_summary(&tests);
        assert_eq!(summary.errors.len(), 1);
        assert_eq!(summary.errors[0].category, "AllographDbQuery");
        assert_eq!(summary.errors[0].tests.module, "allograph");
        assert_eq!(summary.errors[0].tests.count, 1);
        assert_eq!(summary.tests.total, 1);
        assert_eq!(summary.tests.passed, 0);
        assert_eq!(summary.tests.failed, 1);
    }

    #[test]
    fn test_build_summary_with_panic() {
        let tests = vec![make_test("test_panic", "allograph::schema", false)];
        let summary = build_summary(&tests);
        assert_eq!(summary.errors.len(), 1);
        assert_eq!(summary.errors[0].category, "Panic");
        assert_eq!(summary.errors[0].tests.module, "allograph");
        assert_eq!(summary.tests.failed, 1);
    }

    #[test]
    fn test_build_summary_json_shape() {
        let t1 = make_test("test_ok", "allograph::auth", true);
        let mut t2 = make_test("test_bad", "allograph::auth::claims", false);
        t2.panicked = false;
        t2.panic_message = None;
        t2.has_errors = true;
        t2.status = "fail".to_string();
        t2.error = Some("AdminSdl".to_string());
        t2.log_events = vec![LogEvent::SpanEvent {
            message: "err".into(),
            span_name: "test_bad".into(),
            target: "allograph::auth::claims".into(),
            level: "ERROR".into(),
            timestamp: chrono::Utc::now(),
            time_busy: None,
            time_idle: None,
            error: Some("AdminSdl { message: \"type conflict\" }".into()),
        }];
        let tests = vec![t1, t2];
        let summary = build_summary(&tests);
        let json = serde_json::to_value(&summary).unwrap();
        assert!(json.get("errors").is_some());
        assert!(json.get("tests").is_some());
        assert_eq!(json["tests"]["total"], 2);
        assert_eq!(json["tests"]["passed"], 1);
        assert_eq!(json["tests"]["failed"], 1);
        assert_eq!(json["errors"][0]["category"], "AdminSdl");
        assert_eq!(json["errors"][0]["tests"]["module"], "allograph");
    }

    #[test]
    fn test_build_module_tree_three_levels() {
        let tree = build_module_tree("allograph::auth::claims", "test_third", "assertion failed");
        assert_eq!(tree.module, "allograph");
        assert_eq!(tree.count, 1);
        assert_eq!(tree.child.len(), 1);

        let auth = &tree.child[0];
        assert_eq!(auth.module, "auth");
        assert_eq!(auth.count, 1);
        assert_eq!(auth.child.len(), 1);

        let claims = &auth.child[0];
        assert_eq!(claims.module, "claims");
        assert_eq!(claims.count, 1);
        assert_eq!(claims.test.as_deref(), Some("test_third"));
        assert_eq!(claims.message.as_deref(), Some("assertion failed"));
        assert!(claims.child.is_empty());
    }

    #[test]
    fn test_build_module_tree_single_segment() {
        let tree = build_module_tree("allograph", "test_foo", "msg");
        assert_eq!(tree.module, "allograph");
        assert_eq!(tree.count, 1);
        assert_eq!(tree.test.as_deref(), Some("test_foo"));
        assert!(tree.child.is_empty());
    }

    #[test]
    fn test_build_module_tree_multiple_tests_same_module() {
        let mut tree = build_module_tree("allograph::auth::claims", "test_a", "msg_a");
        let idx = tree.child[0].find_or_create_child_idx("claims");
        tree.child[0].child[idx].insert_test("test_b", "msg_b");
        propagate_counts(&mut tree);

        assert_eq!(tree.count, 2);
        assert_eq!(tree.child[0].count, 2);
        assert_eq!(tree.child[0].child[0].count, 2);
    }

    #[test]
    fn test_build_module_tree_empty_string() {
        let tree = build_module_tree("", "test_x", "msg");
        assert_eq!(tree.module, "unknown");
        assert_eq!(tree.count, 1);
    }

    #[test]
    fn test_module_tree_serialization() {
        let tree = build_module_tree("allograph::auth::claims", "test_third", "assertion failed");
        let json = serde_json::to_value(&tree).unwrap();
        assert_eq!(json["module"], "allograph");
        assert_eq!(json["count"], 1);
        assert!(json.get("test").is_none());
        assert!(json.get("message").is_none());
        assert_eq!(json["child"][0]["module"], "auth");
        assert_eq!(json["child"][0]["child"][0]["module"], "claims");
        assert_eq!(json["child"][0]["child"][0]["test"], "test_third");
        assert_eq!(json["child"][0]["child"][0]["message"], "assertion failed");
    }

    #[test]
    fn test_extract_error_type_struct_variant() {
        let input = r#"AllographDbQuery { message: "unique constraint violation: duplicate key" }"#;
        let (category, message) = extract_error_type(input).unwrap();
        assert_eq!(category, "AllographDbQuery");
        assert_eq!(message, "unique constraint violation: duplicate key");
    }

    #[test]
    fn test_extract_error_type_admin_sdl() {
        let input = r#"AdminSdl { message: "type 'User' already defined" }"#;
        let (category, message) = extract_error_type(input).unwrap();
        assert_eq!(category, "AdminSdl");
        assert_eq!(message, "type 'User' already defined");
    }

    #[test]
    fn test_extract_error_type_user_input() {
        let input = r#"UserInput { message: "missing required field 'sub'" }"#;
        let (category, message) = extract_error_type(input).unwrap();
        assert_eq!(category, "UserInput");
        assert_eq!(message, "missing required field 'sub'");
    }

    #[test]
    fn test_extract_error_type_no_match_plain_string() {
        let input = "some plain error message";
        assert!(extract_error_type(input).is_none());
    }

    #[test]
    fn test_extract_error_type_no_match_no_message_field() {
        let input = r#"SomeError { code: 42 }"#;
        assert!(extract_error_type(input).is_none());
    }

    #[test]
    fn test_extract_error_type_empty_string() {
        assert!(extract_error_type("").is_none());
    }

    #[test]
    fn test_extract_error_type_message_with_escaped_quotes() {
        let input = r#"UserInput { message: "invalid value: \"foo\"" }"#;
        let (category, message) = extract_error_type(input).unwrap();
        assert_eq!(category, "UserInput");
        assert_eq!(message, r#"invalid value: \"foo\""#);
    }

    #[test]
    fn test_concurrent_parser_two_tests() {
        // Note: With the new format, tests are contiguous blocks, not interleaved
        let mut parser = TestParser::new();
        
        // Test A
        parser.parse_line(r#"{"event":"test.start","name":"test_a","module":"m::a","status":"pass","error":null}"#).unwrap();
        parser.parse_line(r#"{"timestamp":"2026-01-01T00:00:01Z","level":"TRACE","fields":{"message":"work_a"},"target":"m::a","span":{"name":"span_a"}}"#).unwrap();
        parser.parse_line(r#"{"event":"test.end","name":"test_a","module":"m::a","duration_ms":10.5}"#).unwrap();
        
        // Test B
        parser.parse_line(r#"{"event":"test.start","name":"test_b","module":"m::b","status":"pass","error":null}"#).unwrap();
        parser.parse_line(r#"{"timestamp":"2026-01-01T00:00:02Z","level":"TRACE","fields":{"message":"work_b"},"target":"m::b","span":{"name":"span_b"}}"#).unwrap();
        parser.parse_line(r#"{"event":"test.end","name":"test_b","module":"m::b","duration_ms":15.0}"#).unwrap();

        let results = parser.finalize();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].name, "test_a");
        assert_eq!(results[0].log_events.len(), 1);
        assert_eq!(results[0].duration_ms, Some(10.5));
        assert_eq!(results[1].name, "test_b");
        assert_eq!(results[1].log_events.len(), 1);
        assert_eq!(results[1].duration_ms, Some(15.0));
    }

    #[test]
    fn test_concurrent_parser_interleaved() {
        // Note: With the new format, tests are contiguous blocks
        let mut parser = TestParser::new();
        
        // Test t1
        parser.parse_line(r#"{"event":"test.start","name":"t1","module":"m","status":"pass","error":null}"#).unwrap();
        parser.parse_line(r#"{"timestamp":"2026-01-01T00:00:01Z","level":"TRACE","fields":{"message":"e1"},"target":"m","span":{"name":"s"}}"#).unwrap();
        parser.parse_line(r#"{"timestamp":"2026-01-01T00:00:02Z","level":"TRACE","fields":{"message":"e3"},"target":"m","span":{"name":"s"}}"#).unwrap();
        parser.parse_line(r#"{"event":"test.end","name":"t1","module":"m","duration_ms":10.0}"#).unwrap();
        
        // Test t2
        parser.parse_line(r#"{"event":"test.start","name":"t2","module":"m","status":"pass","error":null}"#).unwrap();
        parser.parse_line(r#"{"timestamp":"2026-01-01T00:00:03Z","level":"TRACE","fields":{"message":"e2"},"target":"m","span":{"name":"s"}}"#).unwrap();
        parser.parse_line(r#"{"event":"test.end","name":"t2","module":"m","duration_ms":5.0}"#).unwrap();

        let results = parser.finalize();
        assert_eq!(results.len(), 2);
        let t1 = results.iter().find(|t| t.name == "t1").unwrap();
        let t2 = results.iter().find(|t| t.name == "t2").unwrap();
        assert_eq!(t1.log_events.len(), 2);
        assert_eq!(t2.log_events.len(), 1);
    }

    #[test]
    fn test_unterminated_test_not_passed() {
        let mut parser = TestParser::new();
        parser.parse_line(
            r#"{"event":"test.start","name":"t1","module":"m","status":"pass","error":null}"#,
        ).unwrap();
        let results = parser.finalize();
        assert_eq!(results.len(), 1);
        assert!(!results[0].is_passed());
        assert!(results[0].ended_at.is_none());
    }

    #[test]
    fn test_ignored_test() {
        let mut parser = TestParser::new();
        parser.parse_line(
            r#"{"event":"test.start","name":"t1","module":"m","status":"pass","error":null}"#,
        ).unwrap();
        parser.parse_line(
            r#"{"event":"test.ignored","name":"t1","module":"m"}"#,
        ).unwrap();
        parser.parse_line(
            r#"{"event":"test.end","name":"t1","module":"m","duration_ms":5.0}"#,
        ).unwrap();
        let results = parser.finalize();
        assert_eq!(results.len(), 1);
        assert!(results[0].ignored);
        assert!(!results[0].is_passed());
    }
}

impl LogEvent {
    pub fn unwrap_span_event(self) -> SpanEventData {
        match self {
            LogEvent::SpanEvent {
                message,
                span_name,
                target,
                level,
                timestamp,
                time_busy,
                time_idle,
                error,
            } => SpanEventData {
                message,
                span_name,
                target,
                level,
                timestamp,
                time_busy,
                time_idle,
                error,
            },
        }
    }
}

#[derive(Debug, Clone)]
pub struct SpanEventData {
    pub message: String,
    pub span_name: String,
    pub target: String,
    pub level: String,
    pub timestamp: DateTime<chrono::Utc>,
    pub time_busy: Option<String>,
    pub time_idle: Option<String>,
    pub error: Option<String>,
}

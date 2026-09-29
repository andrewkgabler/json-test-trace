#[cfg(feature = "cli")]
use clap::{Parser, Subcommand, ValueEnum};
#[cfg(feature = "cli")]
use std::fs::File;
#[cfg(feature = "cli")]
use std::io::{self, BufRead, BufReader, Write};
#[cfg(feature = "cli")]
use std::path::PathBuf;

#[cfg(feature = "cli")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum OutputMode {
    Full,
    Summary,
}

#[cfg(feature = "cli")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum StatusFilter {
    Pass,
    Fail,
    Panic,
    Ignore,
}

#[cfg(feature = "cli")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

#[cfg(feature = "cli")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum OutputFormat {
    Summary,
    Pretty,
    Json,
    Instructions,
}

#[cfg(feature = "cli")]
#[derive(Parser)]
#[command(name = "json-test-trace", about = "Parse JSON test logs and display results")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[cfg(feature = "cli")]
#[derive(Subcommand)]
enum Command {
    /// Ingest JSONL from stdin, write to cache file, output results
    Ingest {
        /// Cache file (overwritten each run)
        #[arg(long)]
        cache_file: PathBuf,

        /// Output mode: full (default) or summary
        #[arg(long, value_enum, default_value = "full")]
        output: OutputMode,
    },
    /// Query a JSONL file with filters
    Query {
        /// Path to JSONL log file
        input: PathBuf,

        /// Filter by module path (substring match)
        #[arg(long)]
        module: Option<String>,

        /// Filter by full module path (exact match)
        #[arg(long)]
        path: Option<String>,

        /// Filter by test name (substring match)
        #[arg(long)]
        name: Option<String>,

        /// Filter by status
        #[arg(long, value_enum)]
        status: Option<StatusFilter>,

        /// Filter by error type (exact match)
        #[arg(long)]
        error: Option<String>,

        /// Filter by log level
        #[arg(long, value_enum)]
        level: Option<LogLevel>,

        /// Filter by minimum duration (ms)
        #[arg(long)]
        min_duration: Option<f64>,

        /// Filter by maximum duration (ms)
        #[arg(long)]
        max_duration: Option<f64>,

        /// Output format
        #[arg(long, value_enum, default_value = "summary")]
        format: OutputFormat,
    },
}

#[cfg(feature = "cli")]
fn main() {
    let cli = Cli::parse();

    match cli.command {
        Command::Ingest { cache_file, output } => run_ingest(cache_file, output),
        Command::Query {
            input,
            module,
            path,
            name,
            status,
            error,
            level,
            min_duration,
            max_duration,
            format,
        } => run_query(
            input,
            module,
            path,
            name,
            status,
            error,
            level,
            min_duration,
            max_duration,
            format,
        ),
    }
}

#[cfg(feature = "cli")]
fn run_ingest(cache_file: PathBuf, output: OutputMode) {
    let mut parser = json_test_trace::TestParser::new();
    let stdin = io::stdin();

    let mut cache = File::create(&cache_file).unwrap_or_else(|e| {
        eprintln!("Error creating cache file {}: {}", cache_file.display(), e);
        std::process::exit(1);
    });

    let is_full = output == OutputMode::Full;

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(e) => {
                eprintln!("Warning: skipping unreadable line: {}", e);
                continue;
            }
        };
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        cache
            .write_all(line.as_bytes())
            .and_then(|_| cache.write_all(b"\n"))
            .unwrap_or_else(|e| {
                eprintln!("Error writing to cache file: {}", e);
                std::process::exit(1);
            });

        match parser.parse_line(line) {
            Ok(Some(completed)) => {
                if is_full {
                    emit_test_line(&completed);
                }
            }
            Ok(None) => {}
            Err(e) => {
                eprintln!("Error parsing line: {}", e);
            }
        }
    }

    let results = parser.finalize();

    match output {
        OutputMode::Full => {
            let refs: Vec<&json_test_trace::TestResult> = results.iter().collect();
            print_summary(&refs);
        }
        OutputMode::Summary => {
            let summary = json_test_trace::build_summary(&results);
            println!("{}", serde_json::to_string_pretty(&summary).unwrap());
        }
    }
}

#[cfg(feature = "cli")]
fn emit_test_line(test: &json_test_trace::TestResult) {
    let status = test.status_str();
    let duration = test.duration_str();
    eprintln!("[{}] {} ({}) — {}", status, test.name, duration, test.module);
    if test.panicked {
        if let Some(ref msg) = test.panic_message {
            eprintln!("  Panic: {}", msg);
        }
    }
}

#[cfg(feature = "cli")]
fn run_query(
    input: PathBuf,
    module: Option<String>,
    path: Option<String>,
    name: Option<String>,
    status: Option<StatusFilter>,
    error: Option<String>,
    level: Option<LogLevel>,
    min_duration: Option<f64>,
    max_duration: Option<f64>,
    format: OutputFormat,
) {
    let file = File::open(&input).unwrap_or_else(|e| {
        eprintln!("Error reading file {}: {}", input.display(), e);
        std::process::exit(1);
    });
    let reader = BufReader::new(file);

    let mut parser = json_test_trace::TestParser::new();
    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(e) => {
                eprintln!("Warning: skipping unreadable line: {}", e);
                continue;
            }
        };
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Err(e) = parser.parse_line(line) {
            eprintln!("Error parsing line: {}", e);
        }
    }
    let mut results = parser.finalize();

    let filters: Vec<Box<dyn Fn(&json_test_trace::TestResult) -> bool>> = vec![
        module.map(|m| Box::new(move |t: &json_test_trace::TestResult| t.module.contains(m.as_str())) as Box<dyn Fn(&json_test_trace::TestResult) -> bool>),
        path.map(|p| Box::new(move |t: &json_test_trace::TestResult| t.module == p) as Box<dyn Fn(&json_test_trace::TestResult) -> bool>),
        name.map(|n| Box::new(move |t: &json_test_trace::TestResult| t.name.contains(n.as_str())) as Box<dyn Fn(&json_test_trace::TestResult) -> bool>),
        status.map(|s| Box::new(move |t: &json_test_trace::TestResult| match s {
            StatusFilter::Pass => t.is_passed(),
            StatusFilter::Fail => t.has_errors && !t.ignored,
            StatusFilter::Panic => t.panicked,
            StatusFilter::Ignore => t.ignored,
        }) as Box<dyn Fn(&json_test_trace::TestResult) -> bool>),
        error.map(|e| Box::new(move |t: &json_test_trace::TestResult| {
            t.log_events.iter().any(|ev| {
                if let json_test_trace::LogEvent::SpanEvent { error: Some(err), .. } = ev {
                    json_test_trace::extract_error_type(err)
                        .map(|(cat, _)| cat == e.as_str())
                        .unwrap_or(false)
                } else {
                    false
                }
            })
        }) as Box<dyn Fn(&json_test_trace::TestResult) -> bool>),
        level.map(|l| Box::new(move |t: &json_test_trace::TestResult| {
            let level_str = match l {
                LogLevel::Trace => "trace",
                LogLevel::Debug => "debug",
                LogLevel::Info => "info",
                LogLevel::Warn => "warn",
                LogLevel::Error => "error",
            };
            t.log_events.iter().any(|ev| {
                let json_test_trace::LogEvent::SpanEvent { level: ev_level, .. } = ev;
                ev_level.to_lowercase() == level_str
            })
        }) as Box<dyn Fn(&json_test_trace::TestResult) -> bool>),
        min_duration.map(|min| Box::new(move |t: &json_test_trace::TestResult| t.duration_ms.map(|d| d >= min).unwrap_or(false)) as Box<dyn Fn(&json_test_trace::TestResult) -> bool>),
        max_duration.map(|max| Box::new(move |t: &json_test_trace::TestResult| t.duration_ms.map(|d| d <= max).unwrap_or(false)) as Box<dyn Fn(&json_test_trace::TestResult) -> bool>),
    ]
    .into_iter()
    .flatten()
    .collect();

    for filter in filters {
        results.retain(filter);
    }

    let refs: Vec<&json_test_trace::TestResult> = results.iter().collect();
    match format {
        OutputFormat::Summary => print_summary(&refs),
        OutputFormat::Pretty => print_pretty(&refs),
        OutputFormat::Json => print_json(&refs),
        OutputFormat::Instructions => print_instructions(),
    }
}

#[cfg(feature = "cli")]
fn print_summary(results: &[&json_test_trace::TestResult]) {
    let total = results.len();
    let passed = results.iter().filter(|t| t.is_passed()).count();
    let failed = total - passed;

    println!("Test Results: {} total, {} passed, {} failed", total, passed, failed);
    println!();

    for test in results {
        println!(
            "  [{}] {} ({})",
            test.status_str(), test.name, test.duration_str()
        );

        if !test.module.is_empty() {
            println!("        {}", test.module);
        }

        if test.panicked {
            if let Some(ref msg) = test.panic_message {
                println!("        Panic: {}", msg);
            }
        }

        if test.has_errors && !test.panicked {
            println!("        Errors detected in log");
        }
    }
}

#[cfg(feature = "cli")]
fn print_pretty(results: &[&json_test_trace::TestResult]) {
    for test in results {
        println!("=== {} ===", test.name);
        println!("  Status: {}", test.status_str());
        println!("  Duration: {}", test.duration_str());
        println!("  Module: {}", test.module);
        println!("  Log events: {}", test.log_events.len());

        for event in &test.log_events {
            let json_test_trace::LogEvent::SpanEvent {
                message,
                span_name,
                target,
                level,
                time_busy,
                time_idle,
                error,
                ..
            } = event;
            let timing = match (time_busy, time_idle) {
                (Some(busy), Some(idle)) => format!(" [busy: {}, idle: {}]", busy, idle),
                _ => String::new(),
            };
            let err = error
                .as_ref()
                .map(|e| format!(" ERROR: {}", e))
                .unwrap_or_default();
            println!("    [{}] {} > {}{}{}", level, target, span_name, timing, err);
            println!("      {}", message);
        }

        if test.panicked {
            if let Some(ref msg) = test.panic_message {
                println!("  PANIC: {}", msg);
            }
        }
        println!();
    }
}

#[cfg(feature = "cli")]
fn print_json(results: &[&json_test_trace::TestResult]) {
    let output: Vec<_> = results
        .iter()
        .map(|t| {
            serde_json::json!({
                "name": t.name,
                "module": t.module,
                "status": t.status_str(),
                "duration_ms": t.duration_ms,
                "panicked": t.panicked,
                "panic_message": t.panic_message,
                "has_errors": t.has_errors,
                "ignored": t.ignored,
                "log_events": t.log_events.len()
            })
        })
        .collect();

    println!("{}", serde_json::to_string_pretty(&output).unwrap());
}

#[cfg(feature = "cli")]
fn print_instructions() {
    println!("=== User Stories: Diagnosing Test Failures with json-test-trace ===");
    println!();
    println!("As an agent debugging test failures, I want to:");
    println!();
    println!("1. QUICK OVERVIEW");
    println!("   \"I just ran the test suite and want to know how many tests failed and what went wrong.\"");
    println!("   Command: json-test-trace query runs/test.jsonl --format summary");
    println!("   Output: total/passed/failed counts + list of failing tests with error categories");
    println!();
    println!("2. ERROR CATEGORY ENUMERATION");
    println!("   \"I want to see all error categories and which tests exhibit each one.\"");
    println!("   Command: json-test-trace query runs/test.jsonl --status fail --format json | jq '.[] | select(.has_errors) | {{name, module, error}}'");
    println!("   Output: JSON array of failing tests with their error category");
    println!();
    println!("3. DRILL INTO SPECIFIC ERROR TYPE");
    println!("   \"I want all tests failing with AllographDbQuery errors.\"");
    println!("   Command: json-test-trace query runs/test.jsonl --error AllographDbQuery --format pretty");
    println!("   Output: each affected test with full log events and raw error strings");
    println!();
    println!("4. FULL TRACE FOR ONE TEST");
    println!("   \"I want to see everything that happened in test_foo — every span, every log line.\"");
    println!("   Command: json-test-trace query runs/test.jsonl --name test_foo --format pretty");
    println!("   Output: chronological log events with level, target, span name, timing, errors");
    println!();
    println!("5. RAW JSONL FOR ONE TEST");
    println!("   \"I want the raw JSONL lines for test_bar to parse them myself.\"");
    println!("   Command: awk '/\"event\":\"test.start\".*\"test.name\":\"test_bar\"/,/\"event\":\"test.end\"/' runs/test.jsonl");
    println!("   Output: all JSONL lines between test.start and test.end for that test");
    println!();
    println!("6. PANIC INVESTIGATION");
    println!("   \"I want all tests that panicked, with their panic messages.\"");
    println!("   Command: json-test-trace query runs/test.jsonl --status panic --format pretty");
    println!("   Output: each panicking test with panic message from payload field");
    println!();
    println!("7. MODULE-LEVEL ERROR HEATMAP");
    println!("   \"I want to know which modules have the most errors.\"");
    println!("   Command: json-test-trace query runs/test.jsonl --status fail --format json | jq 'group_by(.module) | map({{module: .[0].module, count: length}}) | sort_by(-.count)'");
    println!("   Output: modules sorted by failing test count");
    println!();
    println!("8. SLOW TESTS");
    println!("   \"I want tests that took longer than 1 second.\"");
    println!("   Command: json-test-trace query runs/test.jsonl --min-duration 1000 --format pretty");
    println!("   Output: slow tests with duration and module");
    println!();
    println!("9. ERROR-LEVEL LOGS IN A MODULE");
    println!("   \"I want tests in auth::claims that logged ERROR-level events.\"");
    println!("   Command: json-test-trace query runs/test.jsonl --path \"allograph::auth::claims\" --level error --format pretty");
    println!("   Output: tests in that module with error-level log lines");
    println!();
    println!("10. COMBINED FILTERS");
    println!("    \"I want slow tests in the persistence module that have errors.\"");
    println!("    Command: json-test-trace query runs/test.jsonl --module persistence --min-duration 500 --status fail --format pretty");
    println!("    Output: tests matching all criteria (AND logic)");
    println!();
    println!("11. IGNORED TESTS");
    println!("    \"I want to see which tests were marked #[json_test_trace::ignore].\"");
    println!("    Command: json-test-trace query runs/test.jsonl --status ignore --format pretty");
    println!("    Output: ignored tests (body not executed, test.ignored event emitted)");
    println!();
    println!("12. JSON FOR PROGRAMMATIC PROCESSING");
    println!("    \"I want structured JSON output to pipe into jq or another tool.\"");
    println!("    Command: json-test-trace query runs/test.jsonl --status fail --format json");
    println!("    Output: array of test objects with name, module, status, duration_ms, panicked, has_errors, log_events count");
}
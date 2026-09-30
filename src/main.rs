mod models;

use clap::{Parser, Subcommand, ValueEnum};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum OutputFormat {
    Json,
    Pretty,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum StatusFilter {
    Pass,
    Fail,
}

#[derive(Parser)]
#[command(name = "json-test-trace", about = "Parse JSON test logs and display results")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Query a JSONL file with filters
    Query {
        /// Path to JSONL log file
        input: PathBuf,

        /// Filter by module path (substring match)
        #[arg(long)]
        path: Option<String>,

        /// Filter by test name (substring match)
        #[arg(long)]
        name: Option<String>,

        /// Filter by status (pass or fail only)
        #[arg(long, value_enum)]
        status: Option<StatusFilter>,

        /// Filter by error type (substring match)
        #[arg(long)]
        error: Option<String>,

        /// Filter by minimum duration (ms)
        #[arg(long)]
        min_duration: Option<f64>,

        /// Filter by maximum duration (ms)
        #[arg(long)]
        max_duration: Option<f64>,

        /// Output format
        #[arg(long, value_enum, default_value = "pretty")]
        format: OutputFormat,
    },
    /// Generate summary JSON from JSONL input
    Summary {
        /// Path to JSONL log file
        input: PathBuf,
    },
    /// Generate instructions for tests
    Instructions {
        /// Path to JSONL log file
        input: PathBuf,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Command::Query {
            input,
            path,
            name,
            status,
            error,
            min_duration,
            max_duration,
            format,
        } => run_query(
            input,
            path,
            name,
            status,
            error,
            min_duration,
            max_duration,
            format,
        ),
        Command::Summary { input } => run_summary(input),
        Command::Instructions { input } => run_instructions(input),
    }
}

fn run_query(
    input: PathBuf,
    path: Option<String>,
    name: Option<String>,
    status: Option<StatusFilter>,
    error: Option<String>,
    min_duration: Option<f64>,
    max_duration: Option<f64>,
    format: OutputFormat,
) {
    let file = File::open(&input).unwrap_or_else(|e| {
        eprintln!("Error reading file {}: {}", input.display(), e);
        std::process::exit(1);
    });
    let reader = BufReader::new(file);

    let mut parser = models::TestParser::new();
    for line in reader.lines() {
        let line = line.unwrap_or_else(|e| {
            eprintln!("Error reading line: {}", e);
            std::process::exit(1);
        });
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Err(e) = parser.parse_line(line) {
            eprintln!("Error parsing line: {}", e);
        }
    }
    let results_owned = parser.finalize();
    let mut results: Vec<&models::TestResult> = results_owned.iter().collect();

    if let Some(ref p) = path {
        results.retain(|t| t.module.contains(p.as_str()));
    }
    if let Some(ref n) = name {
        results.retain(|t| t.name.contains(n.as_str()));
    }
    if let Some(s) = status {
        results.retain(|t| match s {
            StatusFilter::Pass => t.is_passed(),
            StatusFilter::Fail => !t.is_passed(),
        });
    }
    if let Some(ref e) = error {
        results.retain(|t| {
            t.log_events.iter().any(|ev| {
                if let models::LogEvent::SpanEvent { error: Some(err), .. } = ev {
                    err.contains(e.as_str())
                } else {
                    false
                }
            })
        });
    }
    if let Some(min) = min_duration {
        results.retain(|t| t.duration_ms.map(|d| d >= min).unwrap_or(false));
    }
    if let Some(max) = max_duration {
        results.retain(|t| t.duration_ms.map(|d| d <= max).unwrap_or(false));
    }

    match format {
        OutputFormat::Json => print_json(&results),
        OutputFormat::Pretty => print_pretty(&results),
    }
}

fn run_summary(input: PathBuf) {
    let file = File::open(&input).unwrap_or_else(|e| {
        eprintln!("Error reading file {}: {}", input.display(), e);
        std::process::exit(1);
    });
    let reader = BufReader::new(file);

    let mut parser = models::TestParser::new();
    for line in reader.lines() {
        let line = line.unwrap_or_else(|e| {
            eprintln!("Error reading line: {}", e);
            std::process::exit(1);
        });
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Err(e) = parser.parse_line(line) {
            eprintln!("Error parsing line: {}", e);
        }
    }
    let results = parser.finalize();
    let summary = models::build_summary(results);
    println!("{}", serde_json::to_string_pretty(&summary).unwrap());
}

fn run_instructions(input: PathBuf) {
    let file = File::open(&input).unwrap_or_else(|e| {
        eprintln!("Error reading file {}: {}", input.display(), e);
        std::process::exit(1);
    });
    let reader = BufReader::new(file);

    let mut parser = models::TestParser::new();
    for line in reader.lines() {
        let line = line.unwrap_or_else(|e| {
            eprintln!("Error reading line: {}", e);
            std::process::exit(1);
        });
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Err(e) = parser.parse_line(line) {
            eprintln!("Error parsing line: {}", e);
        }
    }
    let results = parser.finalize();
    print_instructions(&results.iter().collect::<Vec<_>>());
}

fn print_summary(results: &[&models::TestResult]) {
    let total = results.len();
    let passed = results.iter().filter(|t| t.is_passed()).count();
    let failed = total - passed;

    println!("Test Results: {} total, {} passed, {} failed", total, passed, failed);
    println!();

    for test in results {
        let status = if test.is_passed() {
            "PASS"
        } else if test.panicked {
            "FAIL (panic)"
        } else {
            "FAIL"
        };

        let duration = test
            .duration_ms
            .map(|d| format!("{:.1}ms", d))
            .unwrap_or_else(|| "N/A".to_string());

        println!(
            "  [{}] {} ({})",
            status, test.name, duration
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

fn print_pretty(results: &[&models::TestResult]) {
    let output: Vec<_> = results
        .iter()
        .map(|t| {
            serde_json::json!({
                "name": t.name,
                "module": t.module,
                "status": if t.is_passed() { "PASS" } else if t.panicked { "FAIL (panic)" } else { "FAIL" },
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

fn print_json(results: &[&models::TestResult]) {
    let output: Vec<_> = results
        .iter()
        .map(|t| {
            serde_json::json!({
                "name": t.name,
                "module": t.module,
                "status": if t.is_passed() { "PASS" } else if t.panicked { "FAIL (panic)" } else { "FAIL" },
                "duration_ms": t.duration_ms,
                "panicked": t.panicked,
                "panic_message": t.panic_message,
                "has_errors": t.has_errors,
                "ignored": t.ignored,
                "log_events": t.log_events.len()
            })
        })
        .collect();

    // Output as JSONL (one JSON object per line)
    for item in &output {
        println!("{}", serde_json::to_string(item).unwrap());
    }
}

fn print_instructions(results: &[&models::TestResult]) {
    for test in results {
        let status = if test.is_passed() {
            "PASS"
        } else if test.panicked {
            "FAIL (panic)"
        } else {
            "FAIL"
        };

        println!("=== {} ===", test.name);
        println!("  Status: {}", status);
        println!("  Module: {}", test.module);
        
        if test.panicked {
            if let Some(ref msg) = test.panic_message {
                println!("  Panic: {}", msg);
            }
        }
        
        if test.has_errors && !test.panicked {
            println!("  Errors: Test failed with errors in log");
        }
        
        println!();
    }
}
# json-test-trace

A fork of [test-log](https://github.com/d-e-s-o/test-log) that automatically initializes tracing for Rust tests with JSON output support.

## Features

- `#[json_test_trace::test]` attribute macro — initializes tracing per-test
- **JSON output** — all logs are JSON by default
- **Test lifecycle events** — emits `test.start` and `test.end` events
- **Drop guard** — `test.end` is emitted even when a test panics
- **Error capture** — panic hook extracts error_enum/error_message from panic messages

## Usage

```rust
#[json_test_trace::test]
fn it_works() {
    info!("Checking whether it still works...");
    assert_eq!(2 + 2, 4);
}

#[json_test_trace::test(tokio::test)]
async fn it_still_works() {
    // ...
}
```

## Error Capture

When tests panic with GraphQL errors, the panic hook extracts structured error info:

```
GraphQL error: [UserInput { message: "duplicate @id value: user-123" }]
```

This produces:
- `error_enum`: "UserInput"
- `error_message`: "duplicate @id value: user-123"

These fields are injected into all JSONL records for the test (test.start, test.end, and all buffered events).

### Required Panic Format

Test helpers must panic with the error enum Debug format:

```rust
panic!("GraphQL error: {:?}", response.errors);
```

### Example JSONL Output

```json
{"event":"test.start","name":"test_graphql_error","status":"fail","error_enum":"UserInput","error_message":"duplicate @id value: user-123",...}
{"level":"ERROR","message":"GraphQL error in response","error_enum":"UserInput","error_message":"duplicate @id value: user-123","error_code":"user-input",...}
{"event":"test.end","name":"test_graphql_error","status":"fail","error_enum":"UserInput","error_message":"duplicate @id value: user-123",...}
```

### Summary Output

The `summary` subcommand groups errors by `error_enum`:

```json
{
  "tests": { "total": 1, "failed": 1 },
  "errors": [
    {
      "error_enum": "UserInput",
      "count": 1,
      "messages": ["duplicate @id value: user-123"]
    }
  ]
}
```

## Environment Variables

| Variable | Description |
|----------|-------------|
| `TEST_LOG_FILE` | Path to JSONL output file (default: `/tmp/json-test-trace.jsonl`) |

## JSON Output Format

Each log line is a JSON object:

```json
{"timestamp":1790814889614061000,"level":"INFO","message":"GraphQL error in response","error_enum":"UserInput","error_message":"duplicate @id value: user-123","error_code":"user-input","test.name":"test_graphql_error","test.module":"error_helper_demo"}
```

### Test Lifecycle Events

| Event | `event` field | Description |
|-------|---------------|-------------|
| Test start | `test.start` | Test begins. Includes `test.name`, `test.module`, `error_enum`, `error_message`. |
| Test end | `test.end` | Test completes (normal exit or via drop guard on panic). |

### Error Logging

Errors should be logged with `?e` (Debug format) to include the enum variant name:

```rust
tracing::error!(error = ?e, "DB error");
```

This produces:
```json
{"level":"ERROR","message":"DB error","error":"AllographDbQuery { message: \"unique constraint violation: ...\" }"}
```

For structured error capture, use `error_enum`, `error_message`, `error_code` fields:

```rust
tracing::error!(error_enum = "UserInput", error_message = "duplicate @id value: user-123", error_code = "user-input", "GraphQL error in response");
```

## Features (Cargo)

- `log` (default) — initializes the `log` crate
- `trace` (default) — initializes the `tracing` crate
- `json` — enables JSON output via `tracing-subscriber`

## MSRV

Rust 1.71+
# json-test-trace

A fork of [test-log](https://github.com/d-e-s-o/test-log) that automatically initializes tracing for Rust tests with JSON output support.

## Features

- `#[json_test_trace::test]` attribute macro — initializes tracing per-test
- **JSON output** — set `RUST_LOG_STYLE=json` for newline-delimited JSON logs
- **Span events** — set `RUST_LOG_SPAN_EVENTS=full` for span lifecycle events
- **Test lifecycle events** — emits `test.start` and `test.end` events
- **Drop guard** — `test.end` is emitted even when a test panics

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

## Environment Variables

| Variable | Description |
|----------|-------------|
| `RUST_LOG` | Log level filter (e.g. `trace`, `debug`, `info`) |
| `RUST_LOG_STYLE` | Set to `json` for JSON output (default: pretty) |
| `RUST_LOG_SPAN_EVENTS` | Span events: `new`, `enter`, `exit`, `close`, `active`, `full` (comma-separated) |

## JSON Output Format

With `RUST_LOG_STYLE=json`, each log line is a JSON object:

```json
{"timestamp":"2026-09-22T10:00:00.000000Z","level":"INFO","fields":{"message":"test.start","test.name":"test_parse_valid_sdl","test.module":"allograph::schema::parsing::operations::parser"},"target":"allograph::schema::parsing::operations::parser","span":{"name":"test_parse_valid_sdl"},"spans":[],"threadId":"ThreadId(1)"}
```

### Test Lifecycle Events

| Event | `fields.message` | Description |
|-------|-----------------|-------------|
| Test start | `test.start` | Test begins. Includes `test.name` and `test.module`. |
| Test end | `test.end` | Test completes (normal exit or via drop guard on panic). |
| Panic | `panic` | Test panicked. `payload` contains the panic message. |

### Error Logging

Errors should be logged with `?e` (Debug format) to include the enum variant name:

```rust
tracing::error!(error = ?e, "DB error");
```

This produces:
```json
{"level":"ERROR","fields":{"message":"DB error","error":"AllographDbQuery { message: \"unique constraint violation: ...\" }"}}
```

The variant name (`AllographDbQuery`) is extractable via regex for grouping/filtering in downstream tools.

## Features (Cargo)

- `log` (default) — initializes the `log` crate
- `trace` (default) — initializes the `tracing` crate
- `json` — enables JSON output via `tracing-subscriber`

## MSRV

Rust 1.71+
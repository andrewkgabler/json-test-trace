// Copyright (C) 2019-2024 Daniel Mueller <deso@posteo.net>
// SPDX-License-Identifier: (Apache-2.0 OR MIT)

#![allow(clippy::eq_op)]

use tokio::runtime::Builder;

use tracing::debug;
use tracing::error;
use tracing::info;
use tracing::instrument;


mod something {
  pub type Error = String;
}

use something::Error;


#[json_test_trace::test]
fn without_return_type() {
  assert_eq!(2 + 2, 4);
}

#[json_test_trace::test]
fn with_return_type() -> Result<(), Error> {
  let result: Result<(), Error> = Ok(());
  result
}

#[json_test_trace::test]
#[should_panic(expected = "success")]
fn with_panic() {
  panic!("success")
}

#[json_test_trace::test(tokio::test)]
async fn with_inner_test_attribute_and_async() {
  assert_eq!(async { 42 }.await, 42)
}

#[json_test_trace::test(test_case::test_case(-2, -4))]
fn with_inner_test_attribute_and_test_args(x: i8, y: i8) {
  assert_eq!(x, -2);
  assert_eq!(y, -4);
}

#[json_test_trace::test(test_case::test_case(-2, -4; "my test name"))]
fn with_inner_test_attribute_and_test_args_and_name(x: i8, y: i8) {
  assert_eq!(x, -2);
  assert_eq!(y, -4);
}

#[should_panic]
#[json_test_trace::test(test_case::test_case(-2, -4))]
fn with_inner_test_attribute_and_test_args_and_panic(x: i8, _y: i8) {
  assert_eq!(x, 0);
}

#[instrument]
async fn instrumented(input: usize) -> usize {
  info!("input = {}", input);
  if input == 0 || input == 4 {
    error!("here we go");
  }
  let result = input + 1;
  info!("result = {}", result);
  result
}

/// To run the tracing tests and manually verify the output, run with
/// the `trace` feature:
/// ```sh
/// cargo test --features trace trace_with_custom_runtime -- --nocapture
/// ```
///
/// Log level can be configured via the `RUST_LOG` env variable and span
/// events for `#[instrumented]` can be configured via the
/// `RUST_LOG_SPAN_EVENTS` env variable:
/// ```sh
/// RUST_LOG=debug RUST_LOG_SPAN_EVENTS=full \
///   cargo test --features trace trace_with_custom_runtime -- --nocapture
/// ```
#[json_test_trace::test]
fn trace_with_custom_runtime() {
  let rt = Builder::new_current_thread().build().unwrap();

  rt.block_on(async {
    instrumented(0).await;
    instrumented(1).await;
    debug!("done");
  })
}

#[json_test_trace::test(tokio::test)]
async fn trace_with_tokio_attribute() {
  instrumented(6).await;
  instrumented(4).await;
  debug!("done");
}

#[cfg(feature = "unstable")]
#[json_test_trace::test(tokio::test)]
#[json_test_trace(default_log_filter = "info")]
async fn trace_with_default_log_filter() {
  instrumented(6).await;
  instrumented(4).await;
  debug!("done");
}

#[json_test_trace::test(tokio::test(flavor = "multi_thread", worker_threads = 1))]
async fn trace_with_tokio_attribute_with_arguments() {
  instrumented(6).await;
  instrumented(4).await;
  debug!("done");
}

// A trait containing `map` method that has the potential to cause
// ambiguities in generated initialization code and is here only to
// prevent accidental regressions. In the past we were susceptible to a
// compilation error because generated code was using Iterator::map (but
// not using fully qualified syntax).
#[allow(dead_code)]
trait Foo: Sized {
  fn map(self) {}
}

impl<T> Foo for T {}

/// Make sure that Foo::map does not interfere with generated
/// initialization code.
#[json_test_trace::test]
fn unambiguous_map() {}


/// A module used for testing the `test` attribute after importing it
/// via `use` instead of using fuller qualified syntax.
mod local {
  use super::Error;

  use json_test_trace::test;

  #[test]
  fn without_return_type() {
    assert_eq!(2 + 2, 4);
  }

  #[test]
  fn with_return_type() -> Result<(), Error> {
    let result: Result<(), Error> = Ok(());
    result
  }

  #[test]
  #[should_panic(expected = "success")]
  fn with_panic() {
    panic!("success")
  }

  #[test(tokio::test)]
  async fn with_inner_test_attribute_and_async() {
    assert_eq!(async { 42 }.await, 42)
  }

  #[test(test_case::test_case(-2, -4))]
  fn with_inner_test_attribute_and_test_args(x: i8, y: i8) {
    assert_eq!(x, -2);
    assert_eq!(y, -4);
  }

  #[should_panic]
  #[test(test_case::test_case(-2, -4))]
  fn with_inner_test_attribute_and_test_args_and_panic(x: i8, _y: i8) {
    assert_eq!(x, 0);
  }
}

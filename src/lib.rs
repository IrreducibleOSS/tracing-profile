//! A span based profiler, utilizing the [tracing](https://docs.rs/tracing/latest/tracing/) crate.
//!
//! # Overview
//! This implementation of `tracing_subscriber::Layer<S>` records the time
//! a span took to execute, along with any user supplied metadata and
//! information necessary to construct a call graph from the resulting logs.
//!
//! Multiple `Layer` implementations are provided:
//!     `CsvLayer`: logs data in CSV format
//!     `PrintTreeLayer`: prints a call graph
//!     `PrintPerfCountersLayer`: prints aggregated performance counters for each span.
//!     `PerfettoLayer`: uses a local or system-wide perfetto tracing service to record data.
//!     `IttApiLayer`: logs data in Intel's [ITT API](https://www.intel.com/content/www/us/en/docs/vtune-profiler/user-guide/2023-1/instrumentation-and-tracing-technology-apis.html)
//!     `TracyLayer`: re-exports the `tracing_tracy::TracyLayer`.
//!
//! `init_tracing` is a convenience function that initializes the tracing with the default values
//! depending on the features enabled and environment variables set.
//!
//! ```
//! use tracing::instrument;
//! use tracing::debug_span;
//! use tracing_profile::init_tracing;
//!
//! #[instrument(skip_all, name= "graph_root", fields(a="b", c="d"))]
//! fn entry_point() {
//!     let span = debug_span!("some_span");
//!     let _scope1 = span.enter();
//!
//!     let span2 = debug_span!("another_span", field1 = "value1");
//!     let _scope2 = span2.enter();
//! }
//!
//! fn main() {
//!     // Initialize the tracing with the default values
//!     // Note that the guard must be kept alive for the duration of the program.
//!     let _guard = init_tracing().unwrap();
//!     
//!     entry_point();
//! }
//! ```
//!
//! Note that if `#[instrument]` is used, `skip_all` is recommended. Omitting this will result in
//! all the function arguments being included as fields.
//!
//! # Features
//! The `panic` feature will turn eprintln! into panic!, causing the program to halt on errors.

mod data;
mod env_utils;
mod errors;
mod layers;

#[cfg(feature = "ittapi")]
pub use layers::ittapi::Layer as IttApiLayer;
#[cfg(feature = "perf_counters")]
pub use layers::print_perf_counters::Layer as PrintPerfCountersLayer;
pub use layers::{
    csv::Layer as CsvLayer,
    graph::{Config as PrintTreeConfig, Layer as PrintTreeLayer},
};
#[cfg(feature = "perf_counters")]
pub use {
    perf_event::events::Cache as PerfCacheEvent, perf_event::events::Event as PerfEvent,
    perf_event::events::Hardware as PerfHardwareEvent,
    perf_event::events::Software as PerfSoftwareEvent,
};

#[cfg(feature = "perfetto")]
pub use layers::perfetto::{Layer as PerfettoLayer, PerfettoSettings as PerfettoCategorySettings};
#[cfg(feature = "perfetto")]
pub use perfetto_sys::PerfettoGuard;

#[cfg(feature = "tracy")]
pub use tracing_tracy::TracyLayer;

pub use layers::init_tracing::init_tracing;

#[cfg(test)]
mod tests {
    use std::thread;
    use std::time::Duration;

    use rusty_fork::rusty_fork_test;
    use tracing::{debug_span, event, Level};

    use super::*;

    fn make_spans() {
        event!(name: "event outside of span", Level::DEBUG, {value = 10});

        {
            let span = debug_span!("root span");
            let _scope1 = span.enter();
            thread::sleep(Duration::from_millis(20));

            // child spans 1 and 2 are siblings
            let span2 = debug_span!("child span1", field1 = "value1", perfetto_track_id = 5);
            let scope2 = span2.enter();
            thread::sleep(Duration::from_millis(20));
            drop(scope2);

            let span3 = debug_span!(
                "child span2",
                field2 = "value2",
                value = 20,
                perfetto_track_id = 5,
                perfetto_flow_id = 10
            );
            let _scope3 = span3.enter();

            thread::sleep(Duration::from_millis(20));
            event!(name: "event in span2", Level::DEBUG, {value = 100});

            // child spans 3 and 4 are siblings
            let span = debug_span!("child span3", field3 = "value3");
            let scope = span.enter();
            thread::sleep(Duration::from_millis(20));
            event!(name: "custom event", Level::DEBUG, {field5 = "value5", counter = true, value = 30});
            drop(scope);

            thread::spawn(|| {
                let span = debug_span!("child span5", field5 = "value5");
                let _scope = span.enter();
                thread::sleep(Duration::from_millis(20));
                event!(name: "custom event", Level::DEBUG, {field5 = "value6", counter = true, value = 10});
            }).join().unwrap();

            let span = debug_span!("child span4", field4 = "value4", perfetto_flow_id = 10);
            thread::sleep(Duration::from_millis(20));
            event!(name: "custom event", Level::DEBUG, {field5 = "value5", counter = true, value = 40});
            let scope = span.enter();
            thread::sleep(Duration::from_millis(20));
            drop(scope);
        }
        event!(name: "event after last span", Level::DEBUG, {value = 20});
    }

    // Since tracing_subscriber::registry() is a global singleton, we need to run the tests in separate processes.
    rusty_fork_test! {
        #[test]
        fn all_layers() {
            let _guard = init_tracing().unwrap();

            _ = make_spans();
        }
    }
}

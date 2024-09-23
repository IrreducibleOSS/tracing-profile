# Perfetto-sys
This crate wraps the [perfetto sdk](https://perfetto.dev/docs/instrumentation/tracing-sdk). Use it as follows:

Create a `PerfettoGuard` which will live for the duration of the tracing session via `PerfettoGuard::new`. Two types of backend are supported:
 - `BackendConfig::InProcess` will record only the trace data from the current process. 
 - `BackendConfig::System` will also record system data. To do this kind of tracing the perfetto tools binaries must be available. Note that the `PerfettoGuard` creation and dropping will take some additional time to launch and stop the perfetto processes.
See the [perfetto documentation](https://perfetto.dev/docs/quickstart/linux-tracing#capturing-a-trace) for the details.


To create a span, create a `TraceEvent` via `TraceEvent::new`. The event will persist until the `TraceEvent` is dropped. Using custom [track event arguments](https://perfetto.dev/docs/instrumentation/track-events#track-event-arguments), [track id](https://perfetto.dev/docs/instrumentation/track-events#tracks) and [flow id](https://perfetto.dev/docs/instrumentation/track-events#flows) are supported.

To update a counter value use `set_counter_u64` and `set_counter_f64` methods.

Resources:

* Perfetto [Trace configuration](https://perfetto.dev/docs/concepts/config) documentation

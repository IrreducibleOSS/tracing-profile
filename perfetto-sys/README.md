# Perfetto-sys
This crate wraps the [perfetto sdk](https://perfetto.dev/docs/instrumentation/tracing-sdk). Use it as follows:

Create a `PerfettoGuard` which will live for the duration of the tracing session via `PerfettoGuard::new()`.
To create a span, create a `TraceEvent` via `TraceEvent::new(<event name>)`. The event will persist until the `TraceEvent` is dropped.

The crate allows you to generate compile-time defined categories of events and counters by specifying the path to a JSON file in `PERFETTO_INTERFACE_FILE` environment variable during the build. See `InterfaceData` structure in `build.rs` for reference.

```
data_sources {
    config {
        name: "track_event"
    }
}
```

Note that the `PerfettoGuard` currently causes the thread to block until it establishes a connection to the tracing service.

# In process mode
When using [in-process mode](https://perfetto.dev/docs/instrumentation/tracing-sdk#in-process-mode), all the data will be written to a local file, path to which can be set by passing the argument to `init_perfetto` function. If the value is null `tracing.perfetto-trace` will be used as a default value. When system mode is chosen this argument is ignored.

# System mode

To use [system mode](https://perfetto.dev/docs/instrumentation/tracing-sdk#system-mode), the `traced` service must be available.

Resources:

* Perfetto [Trace configuration](https://perfetto.dev/docs/concepts/config) documentation

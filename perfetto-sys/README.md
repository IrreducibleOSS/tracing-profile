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

# Building the deb package
```
cd deb
dpkg --build perfetto
sudo dpkg -i perfetto
sudo dpkg -r perfetto
```

# In process mode  
When using in-process mode, the environment variable `PERFETTO_OUTPUT` will be used for the output file. If this variable is not set, a file called `tracing.perfetto-trace` will be saved in the current woriking directory.
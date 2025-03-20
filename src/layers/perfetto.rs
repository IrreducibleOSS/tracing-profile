// Copyright 2024 Irreducible Inc.

use perfetto_sys::{BackendConfig, EventData, PerfettoGuard};
use tracing::{
    field::{Field, Visit},
    span,
};

use crate::data::{with_span_storage_mut, CounterValue, CounterVisitor, PerfettoMetadata};
use crate::errors::err_msg;

use crate::utils::*;

/// Default categoties for events and counters.
pub struct PerfettoSettings {
    pub trace_file_path: Option<String>,
    pub buffer_size_kb: Option<usize>,
}

const PERFETTO_CATEGORY_FIELD: &str = "perfetto_category";
const PERFETTO_TRACK_ID_FIELD: &str = "perfetto_track_id";
const PERFETTO_FLOW_ID_FIELD: &str = "perfetto_flow_id";

struct SpanVisitor<'a>(&'a mut EventData);

impl<'a> Visit for SpanVisitor<'a> {
    fn record_str(&mut self, field: &Field, value: &str) {
        match field.name() {
            PERFETTO_CATEGORY_FIELD => self.0.set_category(value),
            field_name => {
                self.0.add_string_arg(field_name, value);
            }
        }
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        match field.name() {
            PERFETTO_TRACK_ID_FIELD => self.0.set_track_id(value),
            PERFETTO_FLOW_ID_FIELD => self.0.set_flow_id(value),
            field_name => {
                self.0.add_u64_field(field_name, value);
            }
        }
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        match field.name() {
            PERFETTO_TRACK_ID_FIELD => self.0.set_track_id(value as _),
            PERFETTO_FLOW_ID_FIELD => self.0.set_flow_id(value as _),
            field_name => {
                self.0.add_i64_field(field_name, value);
            }
        }
    }

    fn record_f64(&mut self, field: &Field, value: f64) {
        self.0.add_f64_field(field.name(), value);
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.0.add_bool_field(field.name(), value);
    }

    fn record_debug(&mut self, _: &Field, _: &dyn std::fmt::Debug) {}
}

/// Perfetto layer for tracing.
///
/// The layer support two types of entities:
/// - spans are converted into perfetto events. The following special fields are supported:
///   - `perfetto_category`: category of the event. If not specified "default" will be used.
///   - `perfetto_track_id`: track id of the event. See perfetto documentation for more details.
///   - `perfetto_flow_id`: flow id of the event. See perfetto documentation for more details.
/// - events with `counter` field are converted into perfetto counters. The following special fields are supported:
///  - `value`: value of the counter, integer or double. Required.
///  - `unit`: unit of the counter. Optional.
///  - `incremental`: if set to true, the counter will be treated as incremental. Optional.
///
/// ```ignore
/// // At the beginning of the program
/// (layer, guard) = PerfettoLayer::new_from_env().unwrap();
///
/// // guard should be kept alive for the duration of the program
/// ```
pub struct Layer {}

impl Layer {
    /// Create a new layer with the settings from the environment.
    /// The following environment variables are used:
    /// - `PERFETTO_TRACE_FILE_PATH`: path to the output trace file. Default: `tracing.perfetto-trace`
    /// - `PERFETTO_FUSE`: if set, the system backend will be used. Otherwise the in-process backend will be used.
    /// - `PERFETTO_BIN_PATH`: path to the perfetto binaries. If not set, the system path will be used. Is used only with the system backend.
    /// - `PERFETTO_CFG_PATH`: path to the perfetto config file. If not set, the default one `config/system_profiling.cfg` will be used. Is used only with the system backend.
    /// - `PERFETTO_BUFFER_SIZE_KB`: size of the buffer in kilobytes. Default: 50 * 1024. Is used only with the in-process backend.
    /// - `PERFETTO_PLATFORM_NAME`: custom platform name. Default: architecture of the CPU that is currently in use.
    pub fn new_from_env() -> Result<(Self, PerfettoGuard), perfetto_sys::Error> {
        let trace_file_patch = match std::env::var("PERFETTO_TRACE_FILE_PATH") {
            Ok(path) => path,
            Err(_) => {
                let timestamp = get_unix_timestamp();
                let branch = get_current_git_branch();
                let platform = std::env::var("PERFETTO_PLATFORM_NAME")
                    .unwrap_or(std::env::consts::ARCH.to_string());

                format!(
                    "{}-{}{}.perfetto-trace",
                    timestamp,
                    platform,
                    branch.map_or(String::new(), |b| format!("-{}", b))
                )
            }
        };

        let backend = match std::env::var("PERFETTO_FUSE") {
            Ok(_) => BackendConfig::System {
                perfetto_bin_path: std::env::var("PERFETTO_BIN_PATH").ok(),
                perfetto_cfg_path: std::env::var("PERFETTO_CFG_PATH").ok(),
            },
            Err(_) => {
                const DEFAULT_BUFFER_SIZE_KB: usize = 50 * 1024;
                let buffer_size_kb = match std::env::var("PERFETTO_BUFFER_SIZE_KB") {
                    Ok(size) => size.parse().unwrap_or(DEFAULT_BUFFER_SIZE_KB),
                    Err(_) => DEFAULT_BUFFER_SIZE_KB,
                };

                BackendConfig::InProcess { buffer_size_kb }
            }
        };

        let guard = PerfettoGuard::new(backend, &trace_file_patch)?;
        Ok((Self {}, guard))
    }
}

impl<S> tracing_subscriber::Layer<S> for Layer
where
    S: tracing::Subscriber,
    // no idea what this is but it lets you access the parent span.
    S: for<'lookup> tracing_subscriber::registry::LookupSpan<'lookup>,
{
    // turns log events into counters
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let mut data = CounterVisitor::default();
        event.record(&mut data);

        if !data.is_counter {
            return;
        }

        match data.value {
            Some(CounterValue::Int(value)) => {
                perfetto_sys::set_counter_u64(
                    event.metadata().name(),
                    data.unit.as_ref().map(String::as_str),
                    data.is_incremental,
                    value,
                );
            }
            Some(CounterValue::Float(value)) => {
                perfetto_sys::set_counter_f64(
                    event.metadata().name(),
                    data.unit.as_ref().map(String::as_str),
                    data.is_incremental,
                    value,
                );
            }
            None => {
                err_msg!(
                    "invalid event(missing either 'name' or 'value'): {:?}",
                    event
                );
                return;
            }
        };
    }

    fn on_record(
        &self,
        _id: &span::Id,
        _values: &span::Record<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
    }

    fn on_new_span(
        &self,
        attrs: &span::Attributes<'_>,
        id: &span::Id,
        ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        match ctx.span(id) {
            Some(span) => {
                let mut event_data = EventData::new(span.name());

                let mut visitor = SpanVisitor(&mut event_data);
                attrs.record(&mut visitor);

                let storage = PerfettoMetadata::new(event_data);
                let mut extensions = span.extensions_mut();
                extensions.insert(storage);
            }
            None => {
                err_msg!("failed to get span on_enter");
                return;
            }
        };
    }

    fn on_enter(&self, id: &span::Id, ctx: tracing_subscriber::layer::Context<'_, S>) {
        with_span_storage_mut::<PerfettoMetadata, _>(id, ctx, |storage| {
            storage.start();
        });
    }

    fn on_exit(&self, id: &span::Id, ctx: tracing_subscriber::layer::Context<'_, S>) {
        with_span_storage_mut::<PerfettoMetadata, _>(id, ctx, |storage| {
            storage.end();
        });
    }
}

// Copyright 2024-2025 Irreducible Inc.

use ittapi::{Domain, Task};
use std::{fmt::Write, sync::Once};
use tracing::span;
use tracing_subscriber::{layer, registry::LookupSpan};

use crate::data::{insert_to_span_storage, with_span_storage_mut, WritingFieldVisitor};
use crate::errors::err_msg;

/// A tracing layer that integrates with Intel's Instrumentation and Tracing Technology (ITT) API.
///
/// # Overview
///
/// The Intel ITT API is a performance profiling and instrumentation framework that allows
/// developers to add custom performance annotations to their code. These annotations can then
/// be visualized and analyzed using Intel VTune Profiler, a performance analysis tool for
/// finding bottlenecks in CPU, GPU, and threading performance.
///
/// This layer bridges the gap between Rust's `tracing` ecosystem and Intel's profiling tools,
/// allowing you to see your application's tracing spans directly within VTune Profiler's
/// timeline and analysis views.
///
/// # How It Works
///
/// The `IttApiLayer` creates one ITT API `Task` for each tracing span in your application.
/// When a span is entered, it begins an ITT task, and when the span is exited, it ends the task.
/// This creates a hierarchical view of your application's execution flow that can be visualized
/// in VTune Profiler alongside CPU usage, threading information, and other performance metrics.
///
/// Span attributes are included in the task name using the format: `span_name(field1=value1, field2=value2)`.
/// This helps identify specific instances of spans when analyzing performance data.
///
/// # Use Cases
///
/// - **Performance Analysis**: Identify which parts of your Rust application consume the most time
/// - **Bottleneck Detection**: See how tracing spans correlate with CPU usage and other metrics
/// - **Threading Analysis**: Understand how spans execute across different threads
/// - **Custom Metrics**: Combine application-level spans with system-level performance data
///
/// # Requirements
///
/// - Intel VTune Profiler must be installed to collect and visualize the data
/// - The `ittapi` feature must be enabled when building this crate
/// - The ITT API runtime libraries must be available (typically installed with VTune)
///
/// # Example
///
/// ```no_run
/// use tracing_profile::IttApiLayer;
/// use tracing_subscriber::prelude::*;
///
/// tracing_subscriber::registry()
///     .with(IttApiLayer::default())
///     .init();
///
/// // Your spans will now be visible in Intel VTune Profiler
/// let span = tracing::info_span!("compute", iterations = 1000);
/// let _guard = span.enter();
/// // ... expensive computation ...
/// ```
///
/// # Performance Overhead
///
/// The ITT API is designed to have minimal overhead when VTune is not actively collecting data.
/// When profiling is not active, the API calls become lightweight no-ops. However, there is still
/// some overhead from creating task names and managing span data.
///
/// # See Also
///
/// - [Intel ITT API Documentation](https://www.intel.com/content/www/us/en/docs/vtune-profiler/user-guide/2023-1/instrumentation-and-tracing-technology-apis.html)
/// - [Intel VTune Profiler](https://www.intel.com/content/www/us/en/developer/tools/oneapi/vtune-profiler.html)
/// - [ittapi crate](https://github.com/intel/ittapi)
#[derive(Default)]
pub struct Layer;

impl<S: tracing::Subscriber> tracing_subscriber::Layer<S> for Layer
where
    for<'lookup> S: LookupSpan<'lookup>,
{
    fn on_new_span(&self, attrs: &span::Attributes<'_>, id: &span::Id, ctx: layer::Context<'_, S>) {
        let mut full_name = attrs.metadata().name().to_string();

        if !attrs.is_empty() {
            write!(&mut full_name, "(").expect("failed to write");

            let mut visitor = WritingFieldVisitor::new(&mut full_name);
            attrs.record(&mut visitor);

            write!(&mut full_name, ")").expect("failed to write");
        }

        insert_to_span_storage(
            id,
            ctx,
            TaskData {
                name: full_name,
                task: None,
            },
        );
    }

    fn on_enter(&self, id: &span::Id, ctx: layer::Context<'_, S>) {
        with_span_storage_mut::<TaskData, S>(id, ctx, |task_data| {
            task_data.task = Some(Task::begin(global_domain(), task_data.name.as_str()));
        });
    }

    fn on_exit(&self, id: &span::Id, ctx: layer::Context<'_, S>) {
        with_span_storage_mut::<TaskData, S>(id, ctx, |task_data| {
            if let Some(task) = task_data.task.take() {
                task.end();
            } else {
                err_msg!("task not found for span on exit");
            }
        });
    }

    fn on_close(&self, _id: span::Id, _ctx: tracing_subscriber::layer::Context<'_, S>) {}
}

struct TaskData {
    name: String,
    task: Option<Task<'static>>,
}

/// Returns static domain for ittapi tracing
#[allow(static_mut_refs)]
fn global_domain() -> &'static Domain {
    // Unfortunately we can't use `OnceLock` here because `Domain` doesn't implement `Send`.
    // `OnceLock` requires generic type to implement `Send` in order to be `Sync` for the case when
    // it's initialized in one thread and dropped in another, which is not the case for static variables.
    static mut DOMAIN: Option<Domain> = None;
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        // Safety: `DOMAIN` is not initialized yet, only one thread can reach this point.
        unsafe {
            DOMAIN = Some(Domain::new("Global tracing domain"));
        }
    });

    // Safety:
    // - `DOMAIN` is initialized at this point since `Once::call_once` guarantees that all observable effects are visible at this point.
    // - `DOMAIN` is never set to `None` after initialization, so returning reference with static lifetime is safe.
    unsafe { DOMAIN.as_ref().expect("must be initialized") }
}

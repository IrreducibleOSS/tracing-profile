// Copyright 2024-2025 Irreducible Inc.

use ittapi::{Domain, Task};
use std::{fmt::Write, sync::Once};
use tracing::span;
use tracing_subscriber::{layer, registry::LookupSpan};

use crate::data::{insert_to_span_storage, with_span_storage_mut, WritingFieldVisitor};

/// IttApiLayer (internally called layer::ittapi::Layer).
/// This layer creates one `ittapi::Task` per each span.
/// See [ittapi](https://github.com/intel/ittapi).
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
            task_data.task = Some(Task::begin(global_domain(), task_data.name.as_str()))
        });
    }

    fn on_exit(&self, id: &span::Id, ctx: layer::Context<'_, S>) {
        with_span_storage_mut::<TaskData, S>(id, ctx, |task_data| {
            _ = task_data.task.take();
        });
    }

    fn on_close(&self, _id: span::Id, _ctx: tracing_subscriber::layer::Context<'_, S>) {}
}

struct TaskData {
    name: String,
    task: Option<Task<'static>>,
}

/// Returns static domain for ittapi tracing
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

use perfetto_sys::{CounterCategory, CounterValue, EventCategory};
use std::fmt::{self, Debug};
use tracing::{
    field::{Field, Visit},
    span,
};

use crate::data::{with_span_storage_mut, PerfettoMetadata};
use crate::err_msg;

// gets the needed data out of an Event by implementing the Visit trait
#[derive(Default)]
struct CounterVisitor {
    value: Option<CounterValue>,
    is_counter: bool,
}

impl Visit for CounterVisitor {
    fn record_u64(&mut self, field: &Field, value: u64) {
        if field.name() == "value" {
            self.value.replace(CounterValue::Int32(value as i32));
        }
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        if field.name() == "value" {
            self.value.replace(CounterValue::Int32(value as i32));
        }
    }

    fn record_f64(&mut self, field: &Field, value: f64) {
        if field.name() == "value" {
            self.value.replace(CounterValue::Float(value as _));
        }
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        if field.name() == "counter" {
            self.is_counter = value;
        }
    }

    fn record_debug(&mut self, _: &Field, _: &dyn fmt::Debug) {}
}

/// Default categoties for events and counters.
pub struct CategorySettings {
    pub default_span_category: EventCategory,
    pub default_counter_category: CounterCategory,
}

struct CategoryVisitor<Category> {
    category: Option<Category>,
}

impl<Category> Default for CategoryVisitor<Category> {
    fn default() -> Self {
        Self { category: None }
    }
}

impl<Category> Visit for CategoryVisitor<Category>
where
    for<'a> Category: TryFrom<&'a str, Error: Debug>,
{
    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "category" {
            self.category
                .replace(Category::try_from(value).expect("invalid category"));
        }
    }

    fn record_bool(&mut self, _: &Field, _: bool) {}
    fn record_debug(&mut self, _: &Field, _: &dyn fmt::Debug) {}
    fn record_u64(&mut self, _: &Field, _: u64) {}
    fn record_i64(&mut self, _: &Field, _: i64) {}
}

pub struct Layer {
    settings: CategorySettings,
    _perfetto_guard: perfetto_sys::PerfettoGuard,
}

impl Layer {
    pub fn new(backend: crate::PerfettoBackend, settings: CategorySettings) -> Self {
        Self {
            settings,
            _perfetto_guard: perfetto_sys::PerfettoGuard::new(backend),
        }
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

        let Some(value) = data.value else {
            err_msg!(
                "invalid event(missing either 'name' or 'value'): {:?}",
                event
            );
            return;
        };

        let mut visitor = CategoryVisitor::default();
        event.record(&mut visitor);
        let category = visitor
            .category
            .unwrap_or(self.settings.default_counter_category);

        perfetto_sys::update_counter(category, event.metadata().name(), value);
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
                let mut visitor = CategoryVisitor::default();
                attrs.record(&mut visitor);
                let category = visitor
                    .category
                    .unwrap_or(self.settings.default_span_category);

                let storage = PerfettoMetadata::new(span.name(), category);
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

// Copyright 2024-2025 Irreducible Inc.

use std::{
    io::Write,
    ops::{AddAssign, Sub},
    sync::Mutex,
};

use perf_event::{events::Event, Builder, Counter, Group};
use tracing::span;
use tracing_subscriber::{layer, registry::LookupSpan};

use crate::data::{insert_to_span_storage, with_span_storage, with_span_storage_mut};

#[derive(Debug, Default)]
struct PerfCountersValues(Vec<u64>);

impl Sub<&PerfCountersValues> for &PerfCountersValues {
    type Output = PerfCountersValues;

    fn sub(self, rhs: &PerfCountersValues) -> Self::Output {
        PerfCountersValues(
            self.0
                .iter()
                .zip(rhs.0.iter())
                .map(|(a, b)| a - b)
                .collect(),
        )
    }
}

impl AddAssign<&PerfCountersValues> for PerfCountersValues {
    fn add_assign(&mut self, rhs: &PerfCountersValues) {
        self.0
            .iter_mut()
            .zip(rhs.0.iter())
            .for_each(|(a, b)| *a += b);
    }
}

struct PerfCountersData {
    group: Group,
    counters: Vec<Counter>,
}

impl PerfCountersData {
    pub fn new(events: Vec<Event>) -> std::io::Result<Self> {
        let mut group = Group::new()?;
        let counters = events
            .into_iter()
            .map(|event| Builder::new().group(&mut group).kind(event).build())
            .collect::<Result<Vec<_>, _>>()?;

        group.enable()?;

        Ok(Self { group, counters })
    }

    pub fn read(&mut self) -> std::io::Result<PerfCountersValues> {
        let counts = self.group.read()?;

        Ok(PerfCountersValues(
            self.counters.iter().map(|c| counts[c]).collect(),
        ))
    }
}

struct SpanData {
    aggregate: PerfCountersValues,
    last_enter: PerfCountersValues,
}

impl SpanData {
    fn new(size: usize) -> Self {
        Self {
            aggregate: PerfCountersValues(vec![0; size]),
            last_enter: PerfCountersValues(vec![0; size]),
        }
    }

    fn on_enter(&mut self, counters: PerfCountersValues) {
        self.last_enter = counters;
    }

    fn on_exit(&mut self, counters: PerfCountersValues) {
        self.aggregate += &(&counters - &self.last_enter);
    }

    fn print_table(&self, field_names: &[String], out: &mut impl Write) -> std::io::Result<()> {
        for (name, value) in field_names.iter().zip(self.aggregate.0.iter()) {
            writeln!(out, "    {}: {}", name, value)?;
        }

        Ok(())
    }
}

struct PerfCountersInner {
    names: Vec<String>,
    counters: PerfCountersData,
}

impl PerfCountersInner {
    pub fn new(events: Vec<(String, Event)>) -> std::io::Result<Self> {
        Ok(Self {
            names: events.iter().map(|(name, _)| name.clone()).collect(),
            counters: PerfCountersData::new(events.into_iter().map(|(_, event)| event).collect())?,
        })
    }
}

/// PrintPerfCountersLayer (internally called layer::print_perf_counters::Layer)
/// This Layer prints a table with performance counters to stdout
///
/// example output:
/// ```bash
/// cargo test all_layers -- --nocapture
///
/// child span4:
///     instructions: 44142
///     cycles: 34398
/// child span3:
///     instructions: 44132
///     cycles: 37674
/// child span2:
///     instructions: 282256
///     cycles: 272064
/// child span1:
///     instructions: 49107
///     cycles: 112554
/// root span:
///     instructions: 661552
///     cycles: 738894
/// test tests::all_layers ... ok
/// ```
pub struct Layer {
    inner: Mutex<PerfCountersInner>,
}

impl Layer {
    pub fn new(events: Vec<(String, Event)>) -> std::io::Result<Self> {
        Ok(Self {
            inner: Mutex::new(PerfCountersInner::new(events)?),
        })
    }
}

impl<S: tracing::Subscriber> tracing_subscriber::Layer<S> for Layer
where
    for<'lookup> S: LookupSpan<'lookup>,
{
    fn on_new_span(
        &self,
        _attrs: &span::Attributes<'_>,
        id: &span::Id,
        ctx: layer::Context<'_, S>,
    ) {
        insert_to_span_storage(
            id,
            ctx,
            SpanData::new(self.inner.lock().unwrap().names.len()),
        );
    }

    fn on_enter(&self, id: &span::Id, ctx: layer::Context<'_, S>) {
        let mut inner = self.inner.lock().unwrap();
        with_span_storage_mut::<SpanData, _>(id, ctx, |storage| {
            storage.on_enter(inner.counters.read().expect("failed to read perf counters"));
        });
    }

    fn on_exit(&self, id: &span::Id, ctx: layer::Context<'_, S>) {
        let mut inner = self.inner.lock().unwrap();
        with_span_storage_mut::<SpanData, _>(id, ctx, |storage| {
            storage.on_exit(inner.counters.read().expect("failed to read perf counters"));
        });
    }

    fn on_close(&self, id: span::Id, ctx: tracing_subscriber::layer::Context<'_, S>) {
        println!("{}:", ctx.span(&id).expect("span not found").name());
        with_span_storage::<SpanData, _>(&id, ctx, |storage| {
            storage
                .print_table(&self.inner.lock().unwrap().names, &mut std::io::stdout())
                .expect("failed to print table");
        });
    }
}

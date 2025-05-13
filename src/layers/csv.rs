use linear_map::LinearMap;
use nix::sys::time::TimeValLike;
use nix::time::{clock_gettime, ClockId};
use std::fmt;
use std::io::Write;
use std::path::Path;
use std::sync::mpsc;
use std::time::Instant;
use tracing::{
    field::{Field, Visit},
    span,
};

use crate::data::{with_span_storage_mut, CsvMetadata, StoringFieldVisitor};
use crate::errors::err_msg;

/// CsvLayer (internally called layer::csv)  
/// This Layer emits logs in CSV format, allowing for fine grained analysis.
///
/// example post processing script:
/// ```python3
/// #!/usr/bin/python3
/// import pandas as pd
/// import numpy as np
///
/// def parse_column(str):
///     try:
///         s = str.replace(';',',')
///         return json.loads(s)
///     except Exception as e:
///         print(e)
///         return None
///
/// df = pd.read_csv("log_file.csv", converters={'metadata': parse_column, 'elapsed_ns': lambda x: np.uint64(x)}))
/// id_to_idx = {}
/// id_to_children = {}
///
/// for idx, row in df.iterrows():
///     id_to_idx[row.id] = idx
///     if id_to_children.get(row.parent_id) == None:
///         id_to_children[row.parent_id] = []
///     id_to_children[row.parent_id].append(row.id)
///
/// # todo: search for a row with a specific `row.span_name`, obtain the `row.id`,
/// # and use `id_to_children[row.id]` to traverse the call graph.
/// ```
/// example output
/// ```bash
/// cargo test all_layers
/// # terminal output omitted
/// cat /tmp/output.csv
///
/// span_name,start_ns,elapsed_ns,metadata
/// child span1,202586,31562,{"field1":"value1"}
/// child span3,318204,13639,{"field3":"value3"}
/// child span4,379085,11041,{"field4":"value4"}
/// child span2,296465,145149,{"field2":"value2"}
/// root span,169514,416371,{}
/// ```

#[derive(Default)]
struct CpuTimeEvent {
    map: LinearMap<&'static str, u64>,
}

impl Visit for CpuTimeEvent {
    fn record_u64(&mut self, field: &Field, value: u64) {
        let field_name = field.name();
        *self.map.entry(field_name).or_insert(0) += value;
    }

    fn record_str(&mut self, _: &Field, _: &str) {}
    fn record_i64(&mut self, _: &Field, _: i64) {}
    fn record_bool(&mut self, _: &Field, _: bool) {}
    fn record_debug(&mut self, _: &Field, _: &dyn fmt::Debug) {}
}

pub struct Layer {
    tx: mpsc::Sender<String>,
    init_time: Instant,
}

impl Layer {
    pub fn new<T: AsRef<Path>>(output_file: T) -> Self {
        // this should panic. that way the user doesn't waste a bunch of time running their program just to find out there is no log file.
        let mut f = std::fs::File::create(output_file).expect("CsvLogger failed to open file");
        let (tx, rx) = mpsc::channel::<String>();
        std::thread::spawn(move || {
            let _ = f.write(LogRow::header().as_bytes());
            while let Ok(msg) = rx.recv() {
                let _ = f.write(msg.as_bytes());
            }

            let _ = f.sync_all();
        });
        Self {
            tx,
            init_time: Instant::now(),
        }
    }
}

impl<S> tracing_subscriber::Layer<S> for Layer
where
    S: tracing::Subscriber,
    // no idea what this is but it lets you access the parent span.
    S: for<'lookup> tracing_subscriber::registry::LookupSpan<'lookup>,
{
    // handles log events like debug!
    fn on_event(&self, event: &tracing::Event<'_>, ctx: tracing_subscriber::layer::Context<'_, S>) {
        if event.metadata().name() != "cpu_time" {
            return;
        }

        let span = ctx.current_span();
        let Some(parent_id) = span.id() else {
            eprintln!("CsvLayer failed to get current span for cpu_time event");
            return;
        };
        let mut data = CpuTimeEvent::default();
        event.record(&mut data);

        if let Some(time_ns) = data.map.get("rayon_ns") {
            with_span_storage_mut(parent_id, ctx, |storage: &mut CsvMetadata| {
                storage.rayon_ns += time_ns;
            });
        }
    }

    fn on_record(
        &self,
        id: &span::Id,
        values: &span::Record<'_>,
        ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        with_span_storage_mut(id, ctx, |storage: &mut CsvMetadata| {
            let mut visitor = StoringFieldVisitor(&mut storage.fields);
            values.record(&mut visitor);
        });
    }

    fn on_enter(&self, id: &span::Id, ctx: tracing_subscriber::layer::Context<'_, S>) {
        with_span_storage_mut::<CsvMetadata, _>(id, ctx, |storage| {
            storage
                .start_time
                .replace(self.init_time.elapsed().as_nanos() as u64);
            storage.cpu_start_time.replace(
                clock_gettime(ClockId::CLOCK_THREAD_CPUTIME_ID).expect("failed to get system time"),
            );
        });
    }

    fn on_exit(&self, id: &span::Id, ctx: tracing_subscriber::layer::Context<'_, S>) {
        let rayon_ns = if let Some(span) = ctx.span(id) {
            if let Some(storage) = span.extensions_mut().get_mut::<CsvMetadata>() {
                let end_cpu_time = clock_gettime(ClockId::CLOCK_THREAD_CPUTIME_ID)
                    .expect("failed to get system time");
                let end_time = self.init_time.elapsed().as_nanos() as u64;
                let start_time = storage.start_time.unwrap_or(end_time);

                let mut fields = std::mem::take(&mut storage.fields);
                if storage.rayon_ns > 0 {
                    fields.insert("rayon_ns", storage.rayon_ns.to_string());
                }

                let cpu_diff = (end_cpu_time - storage.cpu_start_time.unwrap_or(end_cpu_time))
                    .num_nanoseconds();
                let mut cpu_ns = if cpu_diff > 0 { cpu_diff as u64 } else { 0_u64 };
                cpu_ns += storage.rayon_ns;

                let log_row = LogRow {
                    span_name: span.name().into(),
                    start_ns: start_time,
                    elapsed_ns: end_time - start_time,
                    cpu_ns,
                    fields,
                };
                let msg = format!("{log_row}\n");
                let _ = self.tx.send(msg);
                storage.rayon_ns
            } else {
                err_msg!("failed to get storage on_exit");
                0
            }
        } else {
            err_msg!("failed to get span on_exit");
            0
        };

        if let Some(parent_id) = ctx.span(id).and_then(|x| x.parent().map(|y| y.id())) {
            with_span_storage_mut(&parent_id, ctx, |storage: &mut CsvMetadata| {
                storage.rayon_ns += rayon_ns;
            });
        }
    }

    fn on_new_span(
        &self,
        attrs: &span::Attributes<'_>,
        id: &span::Id,
        ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let Some(span) = ctx.span(id) else {
            err_msg!("failed to get span on_new_span");
            return;
        };

        let mut storage = CsvMetadata {
            start_time: None,
            cpu_start_time: None,
            rayon_ns: 0,
            fields: LinearMap::new(),
        };

        // warning: the library user must use #[instrument(skip_all)] or else too much data will be logged
        let mut visitor = StoringFieldVisitor(&mut storage.fields);
        attrs.record(&mut visitor);

        let mut extensions = span.extensions_mut();
        extensions.insert(storage);
    }
}

#[derive(Debug)]
struct LogRow {
    span_name: String,
    start_ns: u64,
    elapsed_ns: u64,
    cpu_ns: u64,
    fields: LinearMap<&'static str, String>,
}

impl LogRow {
    fn header<'a>() -> &'a str {
        "span_name,start_ns,elapsed_ns,cpu_ns,metadata\n"
    }
}

impl std::fmt::Display for LogRow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let kv: Vec<_> = self
            .fields
            .iter()
            .map(|(k, v)| format!("\"{k}\":\"{v}\""))
            .collect();
        // desired: a json string that pandas can parse
        // needs the outer quote ' marks to be omitted
        // the comma is replaced with a semicolon to ensure pandas doesn't interpret it as a new column
        let fields = format!("{{{}}}", kv.join("; "));
        write!(
            f,
            "{},{},{},{},{}",
            self.span_name, self.start_ns, self.elapsed_ns, self.cpu_ns, fields
        )
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use rayon::iter::IntoParallelIterator;
    use rayon::prelude::*;
    use rusty_fork::rusty_fork_test;
    use tracing::debug_span;
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::prelude::*;

    // Since tracing_subscriber::registry() is a global singleton, we need to run the tests in separate processes.
    rusty_fork_test! {
        #[test]
        fn cpu_time1() {
            tracing_subscriber::registry()
                .with(Layer::new("/tmp/output1.csv"))
                .init();

            let _scope = debug_span!("parent span").entered();
            for _ in 1..5 {
                std::thread::sleep(Duration::from_secs(1));
            }

            let start = Instant::now();
            while start.elapsed().as_secs() < 1 {}
        }

        #[test]
        fn cpu_time2() {
            tracing_subscriber::registry()
                .with(Layer::new("/tmp/output2.csv"))
                .init();

            let _scope = debug_span!("parent span").entered();
            (0..5).into_par_iter().for_each(|_| {
                let start = Instant::now();
                while start.elapsed().as_secs() < 1 {}
            });

            let start = Instant::now();
            while start.elapsed().as_secs() < 1 {}
        }

        #[test]
        fn cpu_time3() {
            tracing_subscriber::registry()
                .with(Layer::new("/tmp/output3.csv"))
                .init();

            let _scope = debug_span!("parent span").entered();
            (0..5).into_par_iter().for_each(|_| {
                std::thread::sleep(Duration::from_secs(1));
            });

            let start = Instant::now();
            while start.elapsed().as_secs() < 1 {}
        }

        #[test]
        fn cpu_time4() {
            tracing_subscriber::registry()
                .with(Layer::new("/tmp/output4.csv"))
                .init();

            let _scope = debug_span!("parent span").entered();
            let start = Instant::now();
            while start.elapsed().as_secs() < 1 {}

            let _scope2 = debug_span!("child span").entered();

            (0..5).into_par_iter().for_each(|_| {
                let start = Instant::now();
                while start.elapsed().as_secs() < 1 {}
            });

            let start = Instant::now();
            while start.elapsed().as_secs() < 1 {}
        }

        #[test]
        fn cpu_time5() {
            tracing_subscriber::registry()
                .with(Layer::new("/tmp/output5.csv"))
                .init();

            let _scope = debug_span!("parent span").entered();
            let start = Instant::now();
            while start.elapsed().as_secs() < 1 {}

            (0..2).into_par_iter().for_each(|_| {
                let start = Instant::now();
                while start.elapsed().as_secs() < 1 {}
            });

            let _scope2 = debug_span!("child span").entered();

            (0..5).into_par_iter().for_each(|_| {
                let start = Instant::now();
                while start.elapsed().as_secs() < 1 {}
            });

            let start = Instant::now();
            while start.elapsed().as_secs() < 1 {}
        }
    }
}

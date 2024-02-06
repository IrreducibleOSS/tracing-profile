use std::io::Write;
use std::ops::{Deref, DerefMut};
use std::path::Path;
use std::sync::mpsc;
use std::{collections::BTreeMap, time::Instant};
use tracing::span;

use crate::data::{self, FieldVisitor};
use crate::err_msg;

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
/// id,parent_id,elapsed_ns,span_name,file_name,call_depth,metadata
/// 2,1,3194,child span1,src/lib.rs,2,{"field1":"value1"}
/// 4,3,1105,child span3,src/lib.rs,3,{"field3":"value3"}
/// 5,3,1013,child span4,src/lib.rs,3,{"field4":"value4"}
/// 3,1,34166,child span2,src/lib.rs,2,{"field2":"value2"}
/// 1,0,79099,root span,src/lib.rs,1,{}
/// ```

pub struct Layer {
    tx: mpsc::Sender<String>,
}

// if Csvlayer and GraphLayer are used at the same time, there will be a problem if
// they both register an extension of the same type. The newtype pattern is used here
// to avoid that problem.
struct SpanMetadata(data::SpanMetadata);

impl Deref for SpanMetadata {
    type Target = data::SpanMetadata;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for SpanMetadata {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Layer {
    pub fn new<T: AsRef<Path>>(output_file: T) -> Self {
        // this should panic. that way the user doesn't waste a bunch of time running their program just to find out there is no log file.
        let mut f = std::fs::File::create(output_file).expect("CsvLogger failed to open file");
        let (tx, rx) = mpsc::channel::<String>();
        std::thread::spawn(move || {
            let _ = f.write(
                "id,parent_id,elapsed_ns,span_name,file_name,call_depth,metadata\n".as_bytes(),
            );
            while let Ok(msg) = rx.recv() {
                let _ = f.write(msg.as_bytes());
            }

            let _ = f.sync_all();
        });
        Self { tx }
    }
}

impl<S> tracing_subscriber::Layer<S> for Layer
where
    S: tracing::Subscriber,
    // no idea what this is but it lets you access the parent span.
    S: for<'lookup> tracing_subscriber::registry::LookupSpan<'lookup>,
{
    // handles log events like debug!
    fn on_event(
        &self,
        _event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        // don't care about these
    }

    fn on_record(
        &self,
        id: &span::Id,
        values: &span::Record<'_>,
        ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        if let Some(span) = ctx.span(id) {
            if let Some(storage) = span.extensions_mut().get_mut::<SpanMetadata>() {
                let mut visitor = FieldVisitor(&mut storage.fields);
                values.record(&mut visitor);
            } else {
                err_msg!("failed to get storage on_record");
            }
        } else {
            err_msg!("failed to get span on_record");
        }
    }

    fn on_enter(&self, id: &span::Id, ctx: tracing_subscriber::layer::Context<'_, S>) {
        if let Some(span) = ctx.span(id) {
            if let Some(storage) = span.extensions_mut().get_mut::<SpanMetadata>() {
                storage.start_time.replace(Instant::now());
            } else {
                err_msg!("failed to get storage on_enter");
            }
        } else {
            err_msg!("failed to get span on_enter");
        }
    }

    fn on_exit(&self, id: &span::Id, ctx: tracing_subscriber::layer::Context<'_, S>) {
        if let Some(span) = ctx.span(id) {
            let parent = span.parent();
            if let Some(storage) = span.extensions_mut().get_mut::<SpanMetadata>() {
                let elapsed_ns = storage
                    .start_time
                    .map(|x| x.elapsed().as_nanos() as u64)
                    .unwrap_or_default();

                let fields = std::mem::take(&mut storage.fields);

                let log_row = LogRow {
                    id: span.id().into_u64(),
                    parent_id: parent
                        .as_ref()
                        .map(|p| p.id().into_u64())
                        .unwrap_or_default(),
                    span_name: span.name().into(),
                    file_name: span
                        .metadata()
                        .file()
                        .map(|x| x.to_string())
                        .unwrap_or_default(),
                    elapsed_ns,
                    call_depth: storage.call_depth,
                    fields,
                };

                let msg = format!("{log_row}\n");
                let _ = self.tx.send(msg);
            } else {
                err_msg!("failed to get storage on_exit");
            }
        } else {
            err_msg!("failed to get span on_exit");
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

        let parent_call_depth = span
            .parent()
            .as_ref()
            .and_then(|p| p.extensions().get::<SpanMetadata>().map(|x| x.call_depth))
            .unwrap_or_default();

        let mut storage = SpanMetadata(data::SpanMetadata {
            start_time: None,
            call_depth: parent_call_depth + 1,
            fields: BTreeMap::new(),
        });

        // warning: the library user must use #[instrument(skip_all)] or else too much data will be logged
        let mut visitor = FieldVisitor(&mut storage.fields);
        attrs.record(&mut visitor);

        let mut extensions = span.extensions_mut();
        extensions.insert(storage);
    }
}

#[derive(Debug)]
struct LogRow {
    id: u64,
    parent_id: u64,
    span_name: String,
    file_name: String,
    call_depth: u64,
    elapsed_ns: u64,
    fields: BTreeMap<String, String>,
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
            "{},{},{},{},{},{},{}",
            self.id,
            self.parent_id,
            self.elapsed_ns,
            self.span_name,
            self.file_name,
            self.call_depth,
            fields
        )
    }
}

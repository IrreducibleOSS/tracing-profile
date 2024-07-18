// Copyright 2024 Ulvetanna Inc.
use std::{
    collections::{BTreeMap, HashMap},
    sync::Mutex,
    time::Instant,
};

use crate::{
    data::{
        insert_to_span_storage, with_span_storage_mut, GraphMetadata, LogTree, StoringFieldVisitor,
    },
    err_msg,
};
use tracing::span;

#[derive(Debug)]
pub struct Config {
    /// Display anything above this percentage in bold red
    pub attention_above_percent: f64,

    /// Display anything above this percentage in regular white.
    /// Anything below this percentage will be displayed in dim white/gray.
    pub relevant_above_percent: f64,

    /// Anything below this percentage is collapsed into `[...]`.
    /// This is checked after duplicate calls below relevant_above_percent are aggregated.
    pub hide_below_percent: f64,

    /// Whether to display parent time minus time of all children as
    /// `[unaccounted]`. Useful to sanity check that you are measuring all the bottlenecks
    pub display_unaccounted: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            attention_above_percent: 25.0,
            relevant_above_percent: 2.5,
            hide_below_percent: 1.0,
            display_unaccounted: false,
        }
    }
}
/// GraphLayer (internally called layer::graph)
/// This Layer prints a call graph to stdout
///
/// example output:
/// ```bash
/// cargo test all_layers -- --nocapture
///
/// running 1 test
/// root span [ 123.79µs | 100.00% ]
/// ├── child span1 [ 2.88µs | 2.32% ] { field1 = value1 }
/// └── child span2 [ 51.00µs | 41.20% ] { field2 = value2 }
///    ├── child span3 [ 1.88µs | 1.51% ] { field3 = value3 }
///    └── child span4 [ 1.58µs | 1.28% ] { field4 = value4 }
/// test tests::all_layers ... ok
/// ```
pub struct Layer {
    graph: Mutex<TracingGraph>,
}

impl Default for Layer {
    fn default() -> Self {
        Layer::new(Config::default())
    }
}

impl Layer {
    pub fn new(config: Config) -> Self {
        let graph = TracingGraph::new(config).into();
        Self { graph }
    }
}

impl<S> tracing_subscriber::Layer<S> for Layer
where
    S: tracing::Subscriber,
    // no idea what this is but it lets you access the parent span.
    S: for<'lookup> tracing_subscriber::registry::LookupSpan<'lookup>,
{
    fn on_record(
        &self,
        id: &span::Id,
        values: &span::Record<'_>,
        ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        with_span_storage_mut(id, ctx, |storage: &mut GraphMetadata| {
            let mut visitor = StoringFieldVisitor(&mut storage.fields);
            values.record(&mut visitor);
        });
    }

    fn on_enter(&self, id: &span::Id, ctx: tracing_subscriber::layer::Context<'_, S>) {
        with_span_storage_mut(id, ctx, |storage: &mut GraphMetadata| {
            storage.start_time.replace(Instant::now());
        });
    }

    fn on_exit(&self, id: &span::Id, ctx: tracing_subscriber::layer::Context<'_, S>) {
        let Some(span) = ctx.span(id) else {
            return err_msg!("failed to get span on_exit");
        };
        let mut storage = span.extensions_mut();
        let Some(storage) = storage.get_mut::<GraphMetadata>() else {
            return err_msg!("failed to get storage on_exit");
        };

        let graph_node = GraphNode {
            id: span.id().into_u64(),
            execution_duration: storage.start_time.map(|x| x.elapsed()).unwrap_or_default(),
            name: span.name().into(),
            metadata: std::mem::take(&mut storage.fields),
            call_count: 1,
        };

        let Ok(mut graph) = self.graph.lock() else {
            return err_msg!("failed to get mutex");
        };
        match span.parent() {
            Some(p) => {
                graph
                    .children
                    .entry(p.id().into_u64())
                    .or_default()
                    .push(graph_node);
            }
            None => {
                let tree = graph.render_tree(&graph_node, graph_node.execution_duration);
                graph.children.clear();
                println!("{}", tree);
            }
        }
    }

    fn on_new_span(
        &self,
        attrs: &span::Attributes<'_>,
        id: &span::Id,
        ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let mut storage = GraphMetadata {
            start_time: None,
            fields: BTreeMap::new(),
        };
        // warning: the library user must use #[instrument(skip_all)] or else too much data will be logged
        let mut visitor = StoringFieldVisitor(&mut storage.fields);
        attrs.record(&mut visitor);

        insert_to_span_storage(id, ctx, storage);
    }
}

#[derive(Default)]
struct TracingGraph {
    children: HashMap<u64, Vec<GraphNode>>,
    config: Config,
    no_color: bool,
}

impl TracingGraph {
    fn new(config: Config) -> Self {
        Self {
            children: HashMap::new(),
            config,
            no_color: std::env::var("NO_COLOR").map_or(false, |var| !var.is_empty()),
        }
    }

    fn render_tree(&self, node: &GraphNode, root_time: std::time::Duration) -> LogTree {
        let mut children = vec![];
        let mut aggregated_node: Option<GraphNode> = None;
        let mut name_counter: HashMap<&str, usize> = HashMap::new();

        if let Some(unprocessed_children) = self.children.get(&node.id) {
            for (i, child) in unprocessed_children.iter().enumerate() {
                let name_count = name_counter.entry(&child.name).or_insert(0);
                *name_count += 1;

                let next = unprocessed_children.get(i + 1);
                if next.is_some_and(|next| next.name == child.name) {
                    if child.execution_percentage(root_time) > self.config.relevant_above_percent {
                        let mut indexed_child = child.clone();
                        indexed_child
                            .metadata
                            .insert("index".into(), format!("{}", name_count));
                        children.push(indexed_child);
                    } else {
                        aggregated_node = aggregated_node
                            .map(|node| node.clone().aggregate(child))
                            .or_else(|| Some(child.clone()));
                    }
                } else {
                    let child = aggregated_node.take().unwrap_or_else(|| child.clone());
                    children.push(child);
                }
            }
        }

        if self.config.hide_below_percent > 0.0 {
            children = children.into_iter().fold(vec![], |acc, child| {
                let mut acc = acc;
                if child.execution_percentage(root_time) < self.config.hide_below_percent {
                    if let Some(x) = acc.last_mut() {
                        if x.name == "[...]" {
                            *x = x.clone().aggregate(&child);
                        } else {
                            acc.push(GraphNode::new("[...]".into()).aggregate(&child))
                        }
                    }
                } else {
                    acc.push(child);
                }
                acc
            });
        }

        if self.config.display_unaccounted && !children.is_empty() {
            let mut unaccounted = GraphNode::new("[unaccounted]".into());
            unaccounted.execution_duration = node.execution_duration
                - self
                    .children
                    .get(&node.id)
                    .map_or(std::time::Duration::new(0, 0), |children| {
                        children
                            .iter()
                            .map(|x| x.execution_duration)
                            .fold(std::time::Duration::new(0, 0), |x, y| x + y)
                    });
            children.insert(0, unaccounted);
        }

        LogTree {
            label: node.label(root_time, &self.config, self.no_color),
            children: children
                .into_iter()
                .map(|child| self.render_tree(&child, root_time))
                .collect(),
        }
    }
}

#[derive(Default, Debug, Clone)]
struct GraphNode {
    name: String,
    id: u64,
    execution_duration: std::time::Duration,
    metadata: BTreeMap<String, String>,
    call_count: usize,
}

impl GraphNode {
    fn new(name: String) -> Self {
        Self {
            name,
            ..Default::default()
        }
    }

    fn execution_percentage(&self, root_time: std::time::Duration) -> f64 {
        100.0 * self.execution_duration.as_secs_f64() / root_time.as_secs_f64()
    }

    fn label(&self, root_time: std::time::Duration, config: &Config, no_color: bool) -> String {
        let mut info = vec![];
        if self.call_count > 1 {
            info.push(format!("({} calls)", self.call_count))
        } else if !self.metadata.is_empty() {
            let kv: Vec<_> = self
                .metadata
                .iter()
                .map(|(k, v)| format!("{k} = {v}"))
                .collect();
            info.push(format!("{{ {} }}", kv.join(", ")))
        }

        let name = &self.name;
        let execution_time = self.execution_duration;
        let execution_time_percent = self.execution_percentage(root_time);
        let mut result = format!("{name} [ {execution_time:.2?} | {execution_time_percent:.2}% ]");
        if !info.is_empty() {
            result = format!("{result} {}", info.join(" "));
        }

        if no_color {
            result
        } else {
            format!(
                "{}{}\x1b[0m",
                if execution_time_percent > config.attention_above_percent {
                    "\x1b[1;31m" // bold red
                } else if execution_time_percent > config.relevant_above_percent {
                    "\x1b[0m" // white
                } else {
                    "\x1b[2m" // gray
                },
                result
            )
        }
    }

    fn aggregate(mut self, other: &GraphNode) -> Self {
        self.execution_duration += other.execution_duration;
        self.call_count += other.call_count;
        self
    }
}

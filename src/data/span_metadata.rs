use std::{collections::BTreeMap, time::Instant};

use super::EventCounts;
use nix::sys::time::TimeSpec;

#[derive(Debug)]
pub struct CsvMetadata {
    pub start_time: Option<u64>,
    pub cpu_start_time: Option<TimeSpec>,
    pub rayon_ns: u64,
    pub fields: BTreeMap<String, String>,
}

#[derive(Debug)]
#[cfg(feature = "perfetto")]
pub struct PerfettoMetadata {
    label: &'static str,
    category: perfetto_sys::EventCategory,
    trace_guard: Option<perfetto_sys::TraceEvent>,
}

#[cfg(feature = "perfetto")]
impl PerfettoMetadata {
    pub fn new(label: &'static str, category: perfetto_sys::EventCategory) -> Self {
        Self {
            label,
            category,
            trace_guard: None,
        }
    }

    pub fn start(&mut self) {
        self.trace_guard = Some(perfetto_sys::TraceEvent::new(&self.label, self.category));
    }

    pub fn end(&mut self) {
        self.trace_guard = None;
    }
}

#[derive(Debug)]
pub struct GraphMetadata {
    pub start_time: Option<Instant>,
    pub fields: BTreeMap<String, String>,
    pub event_counts: EventCounts,
}

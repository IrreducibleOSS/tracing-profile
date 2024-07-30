use std::{collections::BTreeMap, time::Instant};

use super::EventCounts;

#[derive(Debug)]
pub struct CsvMetadata {
    pub start_time: Option<u64>,
    pub call_depth: u64,
    pub fields: BTreeMap<String, String>,
}

#[derive(Debug)]
pub struct GraphMetadata {
    pub start_time: Option<Instant>,
    pub fields: BTreeMap<String, String>,
    pub event_counts: EventCounts,
}

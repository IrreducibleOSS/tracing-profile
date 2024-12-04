use linear_map::LinearMap;
use nix::sys::time::TimeSpec;

#[derive(Debug)]
pub struct CsvMetadata {
    pub start_time: Option<u64>,
    pub cpu_start_time: Option<TimeSpec>,
    pub rayon_ns: u64,
    pub fields: LinearMap<&'static str, String>,
}

#[cfg(feature = "perfetto")]
pub struct PerfettoMetadata {
    event_data: Option<perfetto_sys::EventData>,
    trace_guard: Option<perfetto_sys::TraceEvent>,
}

#[cfg(feature = "perfetto")]
impl PerfettoMetadata {
    pub fn new(event_data: perfetto_sys::EventData) -> Self {
        Self {
            event_data: Some(event_data),
            trace_guard: None,
        }
    }

    pub fn start(&mut self) {
        self.trace_guard = Some(perfetto_sys::TraceEvent::new(
            self.event_data
                .take()
                .expect("start cannot be called more than once"),
        ));
    }

    pub fn end(&mut self) {
        self.trace_guard = None;
    }
}

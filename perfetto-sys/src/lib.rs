mod counter;
mod error;
mod event;
mod guard;

pub use guard::{BackendConfig, PerfettoGuard};
pub use error::Error;
pub use event::{EventData, TraceEvent};
pub use counter::{set_counter_f64, set_counter_u64};

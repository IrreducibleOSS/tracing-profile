// Copyright 2024-2025 Irreducible Inc.

mod counter;
mod error;
mod event;
mod guard;

pub use counter::{set_counter_f64, set_counter_u64};
pub use error::Error;
pub use event::{create_instant_event, EventData, TraceEvent};
pub use guard::{BackendConfig, PerfettoGuard};

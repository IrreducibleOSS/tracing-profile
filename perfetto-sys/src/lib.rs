// Copyright 2024-2025 Irreducible Inc.

mod counter;
mod error;
mod event;
mod guard;

pub use guard::{BackendConfig, PerfettoGuard};
pub use error::Error;
pub use event::{EventData, TraceEvent, create_instant_event};
pub use counter::{set_counter_f64, set_counter_u64};

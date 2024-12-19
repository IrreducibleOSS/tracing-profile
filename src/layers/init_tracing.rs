// Copyright 2024 Irreducible Inc.

use std::env;

use cfg_if::cfg_if;
use thiserror::Error;
use tracing::{level_filters::LevelFilter, Subscriber};
use tracing_subscriber::filter::EnvFilter;
use tracing_subscriber::{
    filter::Filtered,
    layer::SubscriberExt,
    util::{SubscriberInitExt, TryInitError},
    Layer,
};

use crate::{CsvLayer, PrintTreeConfig, PrintTreeLayer};

trait WithEnvFilter<S: Subscriber>: Layer<S> + Sized {
    fn with_env_filter(self) -> Filtered<Self, EnvFilter, S> {
        let env_level_filter = EnvFilter::builder()
            .with_default_directive(LevelFilter::DEBUG.into())
            .from_env_lossy();

        self.with_filter(env_level_filter)
    }
}

impl<S: Subscriber, T: Layer<S>> WithEnvFilter<S> for T {}

#[derive(Debug, Error)]
pub enum Error {
    #[error("failed to initialize tracing: {0}")]
    TryInit(#[from] TryInitError),
    #[cfg(feature = "perfetto")]
    #[error("failed to initialize Perfetto: {0}")]
    Perfetto(#[from] perfetto_sys::Error),
    #[cfg(feature = "perf_counters")]
    #[error("failed to initialize PerfCounters: {0}")]
    PerfCounters(#[from] std::io::Error),
}

/// Initialize the tracing with the default values depending on the features enabled and environment variables set.
///
/// The following layers are added:
/// - `PrintTreeLayer` (added always)
/// - `IttApiLayer` (added if feature `ittapi` is enabled)
/// - `TracyLayer` (added if feature `tracy` is enabled)
/// - `PrintPerfCountersLayer` (added if feature `perf_counters` is enabled)
/// - `CsvLayer` (added if environment variable `PROFILE_CSV_FILE` is set)
///
/// Returns the guard that should be kept alive for the duration of the program.
pub fn init_tracing() -> Result<impl Drop, Error> {
    // Create print tree layer
    let (layer, guard) = PrintTreeLayer::new(PrintTreeConfig::default());
    let layer = tracing_subscriber::registry().with(layer.with_env_filter());

    // Add perfetto layer if feature is enabled
    let (layer, guard) = {
        cfg_if! {
            if #[cfg(feature = "perfetto")] {
                let (new_layer, new_guard) =
                    crate::PerfettoLayer::new_from_env()?;
                (layer.with(new_layer.with_env_filter()), crate::data::GuardWrapper::wrap(guard, new_guard))
            } else {
                (layer, guard)
            }
        }
    };

    // Add ITT API layer if feature is enabled
    let (layer, guard) = {
        cfg_if! {
            if #[cfg(feature = "ittapi")] {
                (layer.with(crate::IttApiLayer.with_env_filter()), guard)
            } else {
                (layer, guard)
            }
        }
    };

    // Add tracy layer if feature is enabled
    let (layer, guard) = {
        cfg_if! {
            if #[cfg(feature = "tracy")] {
                (layer.with(crate::TracyLayer::default().with_env_filter()), guard)
            } else {
                (layer, guard)
            }
        }
    };

    // Add perf counters layer if feature is enabled
    let (layer, guard) = {
        cfg_if! {
            if #[cfg(feature = "perf_counters")] {
                (layer.with(
                    crate::PrintPerfCountersLayer::new(vec![
                        ("instructions".to_string(), crate::PerfHardwareEvent::INSTRUCTIONS.into()),
                        ("cycles".to_string(), crate::PerfHardwareEvent::CPU_CYCLES.into()),
                    ])?
                    .with_env_filter(),
                ), guard)
            } else {
                (layer, guard)
            }
        }
    };

    if let Ok(csv_path) = env::var("PROFILE_CSV_FILE") {
        let layer = layer.with(CsvLayer::new(csv_path));
        layer.try_init()?;
    } else {
        layer.try_init()?;
    }

    Ok(guard)
}

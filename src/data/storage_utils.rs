use tracing::span;
use tracing_subscriber::registry::LookupSpan;

use crate::errors::err_msg;

/// Perform operation with mutable span storage value.
pub fn with_span_storage_mut<T, S>(
    id: &span::Id,
    ctx: tracing_subscriber::layer::Context<'_, S>,
    f: impl FnOnce(&mut T),
) where
    T: 'static,
    S: tracing::Subscriber,
    for<'lookup> S: LookupSpan<'lookup>,
{
    let Some(span) = ctx.span(id) else {
        return err_msg!("failed to get span");
    };

    let mut extensions = span.extensions_mut();
    let Some(storage) = extensions.get_mut::<T>() else {
        return err_msg!("Failed to get storage");
    };

    f(storage)
}

/// Perform operation with immutable span storage value.
#[cfg(feature = "perf_counters")]
pub fn with_span_storage<T, S: tracing::Subscriber>(
    id: &span::Id,
    ctx: tracing_subscriber::layer::Context<'_, S>,
    f: impl FnOnce(&T),
) where
    T: 'static,
    S: tracing::Subscriber,
    for<'lookup> S: LookupSpan<'lookup>,
{
    let Some(span) = ctx.span(id) else {
        return err_msg!("failed to get span");
    };

    let extensions = span.extensions();
    let Some(storage) = extensions.get::<T>() else {
        return err_msg!("Failed to get storage");
    };

    f(storage)
}

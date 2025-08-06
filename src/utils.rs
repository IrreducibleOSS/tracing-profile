// Copyright 2025 Irreducible Inc.

/// Creates a [`tracing`] event with the resident set size at its peak in megabytes.
///
/// The name of the event is `max rss mib`.
pub fn emit_max_rss() {
    if let Some(max_rss) = get_max_rss() {
        let max_rss_mb = if cfg!(target_os = "linux") {
            // The maxrss is in kbytes for Linux.
            max_rss / 1024
        } else if cfg!(target_os = "macos") {
            // ... and in bytes for BSD/macOS.
            max_rss / 1024 / 1024
        } else {
            // don't risk confusing.
            0
        };
        tracing::event!(
            name: "max rss mib",
            tracing::Level::INFO,
            value = max_rss_mb,
            counter = true
        );
    }
    fn get_max_rss() -> Option<i64> {
        use nix::sys::resource;

        resource::getrusage(resource::UsageWho::RUSAGE_SELF)
            .ok()
            .map(|usage| usage.max_rss())
    }
}

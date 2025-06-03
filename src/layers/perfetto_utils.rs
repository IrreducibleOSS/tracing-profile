// Copyright 2024-2025 Irreducible Inc.

use std::path::PathBuf;

use crate::utils::{sanitize_filename, GitInfo};
use gethostname::gethostname;
use perfetto_sys::{create_instant_event, EventData};

/// Compute where the .perfetto-trace file should live:
///
/// 1. If $PERFETTO_TRACE_FILE_PATH is set, return that PathBuf.
/// 2. Otherwise, build `<timestamp>-<branch>-<commit>[-dirty]-<platform>-<hostname>.perfetto-trace`
///    and (if $PERFETTO_TRACE_DIR is set) prepend that directory.
///
pub(crate) fn compute_trace_path(
    timestamp_filename: String,
    git_info: Option<&GitInfo>,
) -> PathBuf {
    if let Ok(p) = std::env::var("PERFETTO_TRACE_FILE_PATH") {
        return PathBuf::from(p);
    }

    let mut parts = Vec::with_capacity(6);
    parts.push(timestamp_filename);
    if let Some(g) = &git_info {
        parts.push(sanitize_filename(&g.branch));
        parts.push(g.commit_short.clone());
        if !g.is_clean {
            parts.push("dirty".to_string());
        }
    }
    let platform = std::env::var("PERFETTO_PLATFORM_NAME")
        .unwrap_or_else(|_| std::env::consts::ARCH.to_string());
    parts.push(platform);
    let hostname = gethostname().to_string_lossy().to_string();
    parts.push(hostname);

    let fname = format!("{}.perfetto-trace", parts.join("-"));

    let dir = std::env::var("PERFETTO_TRACE_DIR").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(dir).join(fname)
}

pub(crate) fn emit_run_metadata(
    output_path: PathBuf,
    timestamp_iso: String,
    git_info: Option<&GitInfo>,
    metadata: &[(&'static str, String)],
) {
    // Emit metadata
    let mut event_data = EventData::new("metadata:run_info");

    // Timestamps
    event_data.add_string_arg("timestamp", &timestamp_iso);

    // Trace-file name only (no path)
    if let Some(name) = output_path.file_name().and_then(|os| os.to_str()) {
        event_data.add_string_arg("trace_filename", name);
    }

    // Git metadata
    if let Some(g) = &git_info {
        event_data.add_string_arg("git_branch", &g.branch);
        event_data.add_string_arg("git_commit_short", &g.commit_short);
        if let Some(msg) = &g.commit_message {
            event_data.add_string_arg("git_commit_message", msg);
        }
        if let Some(author) = &g.commit_author {
            event_data.add_string_arg("git_commit_author", author);
        }
        if let Some(time) = &g.commit_time {
            event_data.add_string_arg("git_commit_time", time);
        }
        event_data.add_bool_field("git_clean", g.is_clean);
    }

    // Other zero-dependency metadata
    event_data.add_string_arg("crate_version", env!("CARGO_PKG_VERSION"));
    event_data.add_string_arg("os", std::env::consts::OS);
    event_data.add_string_arg("os_family", std::env::consts::FAMILY);
    event_data.add_string_arg("arch", std::env::consts::ARCH);
    if let Ok(host) = gethostname().into_string() {
        event_data.add_string_arg("hostname", &host);
    }

    // Extra metadata from the metadata argument
    for (key, val) in metadata {
        event_data.add_string_arg(key, val);
    }

    // Extra metadata from the environment
    // Format: "key1=val1,key2=val2,..."
    if let Ok(raw) = std::env::var("PERFETTO_EXTRA_METADATA") {
        for segment in raw.split(',') {
            let segment = segment.trim();
            if segment.is_empty() {
                continue;
            }
            let mut parts = segment.splitn(2, '=');
            if let (Some(k), Some(v)) = (parts.next(), parts.next()) {
                let key = k.trim();
                let val = v.trim();

                let key_static: &'static str = Box::leak(key.to_string().into_boxed_str());
                event_data.add_string_arg(key_static, val);
            }
        }
    }

    create_instant_event(event_data);
}

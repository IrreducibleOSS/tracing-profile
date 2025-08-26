// Copyright 2024-2025 Irreducible Inc.

use std::path::PathBuf;

use crate::filename_builder::TraceFilenameBuilder;
use crate::filename_utils::GitInfo;
use gethostname::gethostname;
use perfetto_sys::{create_instant_event, EventData};

/// Compute where the .perfetto-trace file should live using TraceFilenameBuilder.
///
/// Uses TraceFilenameBuilder for configuration. It maintains full backward compatibility
/// with all existing environment variables.
///
/// Environment variable handling:
/// - `PERFETTO_TRACE_FILE_PATH`: complete override
/// - All builder-specific environment variables are handled by the builder itself
///
/// **Important**: This function creates the `.last_perfetto_trace_path` file for
/// compatibility with existing tooling.
pub(crate) fn compute_trace_path_with_builder(
    builder: TraceFilenameBuilder,
) -> Result<PathBuf, crate::filename_builder::FilenameBuilderError> {
    // Build the path using the builder
    let output_path = builder.build()?;

    // Create .last_perfetto_trace_path file for compatibility with existing tooling
    // This matches the behavior in perfetto.rs new_from_env() - fails if write fails
    let output_path_str = output_path.to_string_lossy().to_string();
    std::fs::write(".last_perfetto_trace_path", &output_path_str)
        .map_err(|e| crate::filename_builder::FilenameBuilderError::IoError(e.to_string()))?;

    Ok(output_path)
}

pub(crate) fn emit_run_metadata(
    output_path: PathBuf,
    timestamp_iso: String,
    git_info: Option<&GitInfo>,
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

    create_instant_event(event_data);
}

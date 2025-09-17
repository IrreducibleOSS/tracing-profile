// Copyright 2024-2025 Irreducible Inc.

use chrono::Local;

/// Sample `Local::now()` once and return a pair:
/// 1) `YYYYMMDDTHHmmss` for filenames (ISO 8601 basic format)
/// 2) full RFC3339/ISO timestamp for metadata
pub fn get_formatted_time() -> (String, String) {
    let now = Local::now();
    (now.format("%Y%m%dT%H%M%S").to_string(), now.to_rfc3339())
}

pub fn sanitize_filename(branch: &str) -> String {
    branch
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | '#' | ' ' | '.' => '-',
            _ => c,
        })
        .collect()
}

/// Information about the current Git repository HEAD.
#[derive(Debug)]
pub struct GitInfo {
    /// Current branch name.
    pub branch: String,
    /// Short (7-char) commit hash.
    pub commit_short: String,
    /// Full commit message (first line), if available.
    #[cfg_attr(not(feature = "perfetto"), allow(dead_code))]
    pub commit_message: Option<String>,
    /// Commit author name, if available.
    #[cfg_attr(not(feature = "perfetto"), allow(dead_code))]
    pub commit_author: Option<String>,
    /// Commit timestamp (ISO-8601), if available.
    #[cfg_attr(not(feature = "perfetto"), allow(dead_code))]
    pub commit_time: Option<String>,
    /// Whether the working tree is clean (no uncommitted changes).
    pub is_clean: bool,
}

/// Returns git information captured at compile time.
///
/// This uses git information that was captured during the build process,
/// making it work even in environments where git is not available at runtime
/// (e.g., Android after deployment).
///
/// The values are baked into the binary as string literals during compilation,
/// so no git repository or environment variables are needed at runtime.
pub fn get_git_info() -> Option<GitInfo> {
    // These env!() macros are resolved at compile time and become string literals
    let branch = env!("BUILD_GIT_BRANCH");
    let commit_short = env!("BUILD_GIT_COMMIT_SHORT");
    let is_dirty = env!("BUILD_GIT_DIRTY");
    let commit_message = env!("BUILD_GIT_COMMIT_MESSAGE");
    let commit_author = env!("BUILD_GIT_COMMIT_AUTHOR");
    let commit_time = env!("BUILD_GIT_COMMIT_TIME");

    // Return None if git info wasn't available at build time
    if branch == "unknown" || commit_short == "unknown" {
        return None;
    }

    Some(GitInfo {
        branch: branch.to_string(),
        commit_short: commit_short.to_string(),
        commit_message: if commit_message.is_empty() {
            None
        } else {
            Some(commit_message.to_string())
        },
        commit_author: if commit_author.is_empty() {
            None
        } else {
            Some(commit_author.to_string())
        },
        commit_time: if commit_time.is_empty() {
            None
        } else {
            Some(commit_time.to_string())
        },
        is_clean: is_dirty != "true",
    })
}

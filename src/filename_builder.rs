// Copyright 2024-2025 Irreducible Inc.

//! Flexible builder for generating perfetto trace filenames.
//!
//! The `TraceFilenameBuilder` provides a chainable API for constructing trace filenames
//! with various components like timestamp, git information, system details, and custom metadata.
//! It provides flexible file naming with environment variable overrides.

use std::path::PathBuf;
use thiserror::Error;

use crate::filename_utils::{get_formatted_time, get_git_info, sanitize_filename};

/// Errors that can occur when building trace filenames.
#[derive(Debug, Clone, Error)]
pub enum FilenameBuilderError {
    #[error("I/O error: {0}")]
    IoError(String),
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),
}

/// Builder for constructing perfetto trace filenames with flexible customization.
///
/// # Example
/// ```rust,no_run
/// use tracing_profile::TraceFilenameBuilder;
///
/// let path = TraceFilenameBuilder::new()
///     .name("my_app")
///     .iteration(1)
///     .timestamp()
///     .git_info()
///     .build()?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, Default)]
pub struct TraceFilenameBuilder {
    timestamp: Option<String>,
    name: Option<String>,
    iteration: Option<usize>,
    git_branch: Option<String>,
    git_commit: Option<String>,
    git_dirty: bool,
    hostname: Option<String>,
    platform: Option<String>,
    machine_name: Option<String>,
    thread_mode: Option<String>,
    thread_count: Option<usize>,
    config: Option<String>,
    run_id: Option<String>,
    custom_fields: Vec<(String, String)>,
    output_dir: Option<PathBuf>,
    subdirs: Vec<String>,
    separator: String,
    prefix: Option<String>,
}

impl TraceFilenameBuilder {
    /// Create a new builder with default settings.
    pub fn new() -> Self {
        Self {
            separator: ".".to_string(),
            ..Default::default()
        }
    }

    /// Add current timestamp in default format (YYYYMMDDTHHmmss).
    pub fn timestamp(mut self) -> Self {
        let (timestamp_filename, _) = get_formatted_time();
        self.timestamp = Some(timestamp_filename);
        self
    }

    /// Add custom formatted timestamp.
    pub fn timestamp_custom(mut self, format: &str) -> Self {
        let now = chrono::Local::now();
        self.timestamp = Some(now.format(format).to_string());
        self
    }

    /// Auto-detect and add all git information (branch, commit, dirty status).
    pub fn git_info(mut self) -> Self {
        if let Some(git_info) = get_git_info() {
            self.git_branch = Some(sanitize_filename(&git_info.branch));
            self.git_commit = Some(git_info.commit_short);
            self.git_dirty = !git_info.is_clean;
        }
        self
    }

    /// Set git branch name (will be sanitized).
    pub fn git_branch(mut self, branch: impl Into<String>) -> Self {
        self.git_branch = Some(sanitize_filename(&branch.into()));
        self
    }

    /// Set git commit hash.
    pub fn git_commit(mut self, commit: impl Into<String>) -> Self {
        self.git_commit = Some(commit.into());
        self
    }

    /// Force the dirty flag (add "dirty" to filename).
    pub fn git_dirty(mut self) -> Self {
        self.git_dirty = true;
        self
    }

    /// Auto-detect and add hostname.
    pub fn hostname(mut self) -> Self {
        if let Ok(hostname) = gethostname::gethostname().into_string() {
            self.hostname = Some(hostname);
        }
        self
    }

    /// Auto-detect and add platform information.
    pub fn platform(mut self) -> Self {
        let platform = std::env::var("PERFETTO_PLATFORM_NAME")
            .unwrap_or_else(|_| std::env::consts::ARCH.to_string());
        self.platform = Some(platform);
        self
    }

    /// Set machine name identifier.
    pub fn machine_name(mut self, name: impl Into<String>) -> Self {
        self.machine_name = Some(name.into());
        self
    }

    /// Set thread mode (e.g., "single", "multi", "async").
    pub fn thread_mode(mut self, mode: impl Into<String>) -> Self {
        self.thread_mode = Some(mode.into());
        self
    }

    /// Set thread count.
    pub fn thread_count(mut self, count: usize) -> Self {
        self.thread_count = Some(count);
        self
    }

    /// Set configuration identifier.
    pub fn config(mut self, config: impl Into<String>) -> Self {
        self.config = Some(config.into());
        self
    }

    /// Set run identifier.
    pub fn run_id(mut self, id: impl Into<String>) -> Self {
        self.run_id = Some(id.into());
        self
    }

    /// Set the benchmark/test name.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Set the iteration/sample number.
    pub fn iteration(mut self, iteration: usize) -> Self {
        self.iteration = Some(iteration);
        self
    }

    /// Add a variant description (e.g., "optimized", "baseline", "multi-threaded").
    pub fn variant(mut self, variant: impl Into<String>) -> Self {
        self.custom_fields
            .push(("variant".to_string(), variant.into()));
        self
    }

    /// Create a new builder initialized from environment variables.
    ///
    /// This method creates a TraceFilenameBuilder with the same default configuration
    /// to provide consistent default behavior, ensuring complete
    /// backward compatibility.
    ///
    /// **Default format**: `<timestamp>.<branch>.<commit>[.dirty].<platform>.<hostname>.perfetto-trace`
    ///
    /// This method applies the following configuration:
    /// - Adds current timestamp in `YYYYMMDDTHHmmss` format
    /// - Adds git branch, commit hash, and dirty flag (if in git repository)
    /// - Adds platform information (CPU architecture)  
    /// - Adds hostname
    /// - Uses "." as separator
    /// - Sets extension to ".perfetto-trace"
    ///
    /// All environment variable overrides are still respected when `build()` is called.
    ///
    /// # Example
    /// ```rust,no_run
    /// use tracing_profile::TraceFilenameBuilder;
    ///
    /// let builder = TraceFilenameBuilder::from_env();
    /// let path = builder.build().unwrap();
    /// // Produces: "20250828T103000.main.abc123.dirty.x86_64.hostname.perfetto-trace"
    /// ```
    pub fn from_env() -> Self {
        Self::new().timestamp().git_info().platform().hostname()
    }

    /// Create a builder with default perfetto trace settings.
    pub fn default_perfetto() -> Self {
        Self::new().timestamp().git_info().platform().hostname()
    }

    /// Create a builder configured for benchmark usage.
    pub fn for_benchmark(name: impl Into<String>) -> Self {
        Self::new()
            .name(name)
            .timestamp()
            .git_info()
            .platform()
            .hostname()
    }

    /// Add a subdirectory with an auto-generated run ID.
    ///
    /// Generates a run ID like "20250115T103045-abc1234" combining:
    /// - Current datetime in YYYYMMDDTHHmmss format
    /// - Short git commit hash (or "nogit" if not in a git repo)
    ///
    /// This is useful for organizing traces from different benchmark sessions.
    #[cfg(feature = "gen_filename")]
    pub fn subdir_run_id(self) -> Self {
        use crate::filename_utils::get_git_info;

        let datetime = chrono::Local::now().format("%Y%m%dT%H%M%S").to_string();

        let git_hash = get_git_info()
            .map(|info| info.commit_short)
            .unwrap_or_else(|| "nogit".to_string());

        let run_id = format!("{datetime}-{git_hash}");
        self.subdir(run_id)
    }

    /// Add a custom key-value pair to the filename.
    pub fn add(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.custom_fields.push((key.into(), value.into()));
        self
    }

    /// Add a custom key-value pair only if the value is Some.
    pub fn add_option<T: Into<String>>(mut self, key: impl Into<String>, value: Option<T>) -> Self {
        if let Some(v) = value {
            self.custom_fields.push((key.into(), v.into()));
        }
        self
    }

    /// Add a value from an environment variable if it exists.
    pub fn add_from_env(mut self, key: impl Into<String>, env_var: impl AsRef<str>) -> Self {
        if let Ok(value) = std::env::var(env_var.as_ref()) {
            self.custom_fields.push((key.into(), value));
        }
        self
    }

    /// Set a prefix that will be prepended to the filename (useful for scripts).
    pub fn prepend(mut self, prefix: impl Into<String>) -> Self {
        self.prefix = Some(prefix.into());
        self
    }

    /// Set custom separator (default is ".").
    pub fn separator(mut self, separator: impl Into<String>) -> Self {
        self.separator = separator.into();
        self
    }

    /// Set output directory.
    pub fn output_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.output_dir = Some(dir.into());
        self
    }

    /// Add a subdirectory level.
    pub fn subdir(mut self, subdir: impl Into<String>) -> Self {
        self.subdirs.push(subdir.into());
        self
    }

    /// Add multiple subdirectory levels at once.
    pub fn subdirs<I, S>(mut self, subdirs: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        for subdir in subdirs {
            self.subdirs.push(subdir.into());
        }
        self
    }

    /// Build the final trace file path.
    ///
    /// # Environment Variable Overrides
    ///
    /// - `PERFETTO_TRACE_FILE_PATH`: Complete override for the full path
    /// - `PERFETTO_TRACE_DIR`: Absolute directory override (ignores all subdirs and output_dir)
    /// - `PERFETTO_TRACE_NAME`: Override for trace name
    /// - `PERFETTO_TRACE_ITERATION`: Override for iteration number
    /// - `PERFETTO_PLATFORM_NAME`: Override for platform identification
    /// - `PERFETTO_MACHINE_NAME`: Override for machine name
    ///
    /// # Errors
    ///
    /// Returns an error if directory creation fails or if the path cannot be constructed.
    pub fn build(self) -> Result<PathBuf, FilenameBuilderError> {
        self.build_impl()
    }

    fn build_impl(self) -> Result<PathBuf, FilenameBuilderError> {
        // Check for complete override first
        if let Ok(path) = std::env::var("PERFETTO_TRACE_FILE_PATH") {
            return Ok(PathBuf::from(path));
        }

        // Apply environment variable overrides
        let final_name = std::env::var("PERFETTO_TRACE_NAME")
            .ok()
            .or(self.name.clone());

        let final_iteration = std::env::var("PERFETTO_TRACE_ITERATION")
            .ok()
            .and_then(|s| s.parse().ok())
            .or(self.iteration);

        let final_machine_name = std::env::var("PERFETTO_MACHINE_NAME")
            .ok()
            .or(self.machine_name.clone());

        // Build filename components in order
        let mut parts = Vec::new();

        // Add prefix if specified
        if let Some(prefix) = &self.prefix {
            parts.push(prefix.clone());
        }

        // Add name first (benchmark name)
        if let Some(name) = &final_name {
            parts.push(name.clone());
        }

        // Add custom fields (threading mode, fusion, etc.)
        for (_, value) in &self.custom_fields {
            if !value.is_empty() {
                parts.push(value.clone());
            }
        }

        // Add iteration with "iter" prefix for clarity
        if let Some(iteration) = final_iteration {
            parts.push(format!("iter{iteration}"));
        }

        // Add timestamp
        if let Some(timestamp) = &self.timestamp {
            parts.push(timestamp.clone());
        }

        // Add git information
        if let Some(commit) = &self.git_commit {
            parts.push(commit.clone());
        }
        if let Some(branch) = &self.git_branch {
            parts.push(branch.clone());
        }

        // Add dirty flag if repository has uncommitted changes
        if self.git_dirty {
            parts.push("dirty".to_string());
        }

        // Add platform/architecture
        if let Some(platform) = &self.platform {
            parts.push(platform.clone());
        }

        // Add other optional fields that are less commonly used
        if let Some(machine_name) = &final_machine_name {
            parts.push(machine_name.clone());
        }
        if let Some(hostname) = &self.hostname {
            parts.push(hostname.clone());
        }

        // Build filename
        let filename = if parts.is_empty() {
            "trace.perfetto-trace".to_string()
        } else {
            format!("{}.perfetto-trace", parts.join(&self.separator))
        };

        // Determine output directory
        // If PERFETTO_TRACE_DIR is set, use it as absolute path (ignore subdirs)
        let full_path = if let Ok(env_dir) = std::env::var("PERFETTO_TRACE_DIR") {
            // Environment variable overrides everything - use exactly this directory
            PathBuf::from(env_dir)
        } else {
            // Build path with base directory and subdirectories
            let base_dir = self
                .output_dir
                .clone()
                .unwrap_or_else(|| PathBuf::from("."));

            // Apply subdirectories
            let mut path = base_dir;
            for subdir in &self.subdirs {
                path = path.join(subdir);
            }
            path
        };

        // Create directories if they don't exist
        std::fs::create_dir_all(&full_path).map_err(|e| {
            FilenameBuilderError::IoError(format!("Failed to create directory {full_path:?}: {e}"))
        })?;

        Ok(full_path.join(filename))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusty_fork::rusty_fork_test;
    use std::env;

    #[test]
    fn test_basic_builder() {
        let builder = TraceFilenameBuilder::new();
        let path = builder.build().unwrap();

        // Should at least create a valid path
        assert!(path.to_string_lossy().ends_with(".perfetto-trace"));
    }

    #[test]
    fn test_with_name_and_iteration() {
        let path = TraceFilenameBuilder::new()
            .name("test_benchmark")
            .iteration(5)
            .build()
            .unwrap();

        let filename = path.file_name().unwrap().to_string_lossy();
        assert!(filename.contains("test_benchmark"));
        assert!(filename.contains("5"));
    }

    #[test]
    fn test_custom_separator() {
        let path = TraceFilenameBuilder::new()
            .name("test")
            .iteration(1)
            .separator("_")
            .build()
            .unwrap();

        let filename = path.file_name().unwrap().to_string_lossy();
        assert!(filename.contains("test_iter1"));
    }

    rusty_fork_test! {
        #[test]
        fn test_environment_override() {
            let test_path = "/tmp/custom_trace.perfetto-trace";
            env::set_var("PERFETTO_TRACE_FILE_PATH", test_path);

            let path = TraceFilenameBuilder::new()
                .name("should_be_ignored")
                .build()
                .unwrap();

            assert_eq!(path, PathBuf::from(test_path));
        }
    }

    #[test]
    fn test_subdirectories() {
        let path = TraceFilenameBuilder::new()
            .name("test")
            .output_dir("/tmp/traces")
            .subdir("benchmarks")
            .subdir("sha256")
            .build()
            .unwrap();

        let path_str = path.to_string_lossy();
        assert!(path_str.contains("traces"));
        assert!(path_str.contains("benchmarks"));
        assert!(path_str.contains("sha256"));
    }

    #[test]
    fn test_git_dirty_flag() {
        let path = TraceFilenameBuilder::new()
            .name("test")
            .git_branch("main")
            .git_commit("abc1234")
            .git_dirty()
            .build()
            .unwrap();

        let filename = path.file_name().unwrap().to_string_lossy();
        assert!(filename.contains("dirty"));
    }

    #[test]
    fn test_backward_compatibility_format() {
        // Test that default format matches expected perfetto trace format
        let path = TraceFilenameBuilder::new()
            .timestamp()
            .git_info() // May or may not work depending on test environment
            .platform()
            .hostname()
            .build()
            .unwrap();

        let filename = path.file_name().unwrap().to_string_lossy();

        // Should have basic structure: timestamp.platform.hostname.perfetto-trace
        assert!(filename.ends_with(".perfetto-trace"));

        // Should have separator-delimited components
        let parts: Vec<&str> = filename
            .trim_end_matches(".perfetto-trace")
            .split('.')
            .collect();
        assert!(parts.len() >= 2); // At least timestamp and platform/hostname
    }

    #[test]
    fn test_comprehensive_example() {
        // Test a comprehensive example similar to what benchmarks would use
        let path = TraceFilenameBuilder::new()
            .name("sha256")
            .iteration(1)
            .variant("multi-threaded")
            .timestamp_custom("%Y_%m_%d_%H_%M")
            .git_branch("feature-branch")
            .git_commit("abc1234")
            .git_dirty()
            .platform()
            .hostname()
            .output_dir("./traces")
            .subdir("benchmarks")
            .subdir("sha256")
            .build()
            .unwrap();

        let filename = path.file_name().unwrap().to_string_lossy();
        let full_path = path.to_string_lossy();

        // Check filename components
        assert!(filename.contains("multi-threaded"));
        assert!(filename.contains("sha256"));
        assert!(filename.contains("1"));
        assert!(filename.contains("feature-branch"));
        assert!(filename.contains("abc1234"));
        assert!(filename.contains("dirty"));
        assert!(filename.ends_with(".perfetto-trace"));

        // Check directory structure
        assert!(full_path.contains("traces"));
        assert!(full_path.contains("benchmarks"));
        assert!(full_path.contains("sha256"));
    }

    #[test]
    fn test_convenience_methods() {
        // Test from_env method
        let path0 = TraceFilenameBuilder::from_env().build().unwrap();
        let filename0 = path0.file_name().unwrap().to_string_lossy();
        assert!(filename0.ends_with(".perfetto-trace"));

        // Test default_perfetto method
        let path1 = TraceFilenameBuilder::default_perfetto().build().unwrap();
        let filename1 = path1.file_name().unwrap().to_string_lossy();
        assert!(filename1.ends_with(".perfetto-trace"));

        // Test for_benchmark method
        let path2 = TraceFilenameBuilder::for_benchmark("test_benchmark")
            .iteration(5)
            .build()
            .unwrap();
        let filename2 = path2.file_name().unwrap().to_string_lossy();
        assert!(filename2.contains("test_benchmark"));
        assert!(filename2.contains("5"));
    }

    #[test]
    fn test_add_option() {
        let path1 = TraceFilenameBuilder::new()
            .name("test")
            .add_option("optional", Some("value"))
            .add_option("empty", None::<String>)
            .build()
            .unwrap();

        let filename1 = path1.file_name().unwrap().to_string_lossy();
        assert!(filename1.contains("value"));
        assert!(!filename1.contains("empty"));
    }

    rusty_fork_test! {
        #[test]
        fn test_add_from_env() {
            std::env::set_var("TEST_CUSTOM_VAR", "custom_value");
            std::env::remove_var("TEST_MISSING_VAR");

            let path = TraceFilenameBuilder::new()
                .name("test")
                .add_from_env("custom", "TEST_CUSTOM_VAR")
                .add_from_env("missing", "TEST_MISSING_VAR")
                .build()
                .unwrap();

            let filename = path.file_name().unwrap().to_string_lossy();
            assert!(filename.contains("custom_value"));
            assert!(!filename.contains("missing"));
        }
    }

    rusty_fork_test! {
        #[test]
        fn test_env_variable_overrides() {
            std::env::set_var("PERFETTO_TRACE_NAME", "env_name");
            std::env::set_var("PERFETTO_TRACE_ITERATION", "42");
            std::env::set_var("PERFETTO_MACHINE_NAME", "env_machine");

            let path = TraceFilenameBuilder::new()
                .name("original_name")
                .iteration(1)
                .machine_name("original_machine")
                .build()
                .unwrap();

            let filename = path.file_name().unwrap().to_string_lossy();
            assert!(filename.contains("env_name"));
            assert!(filename.contains("iter42"));
            assert!(filename.contains("env_machine"));
            assert!(!filename.contains("original_name"));
            assert!(!filename.contains("iter1"));
            assert!(!filename.contains("original_machine"));
        }

        #[test]
        fn test_perfetto_trace_dir_absolute() {
            std::env::set_var("PERFETTO_TRACE_DIR", "/tmp/my_traces");

            let path = TraceFilenameBuilder::new()
                .name("test")
                .output_dir("/ignored/path")
                .subdir("also_ignored")
                .subdir("ignored_too")
                .build()
                .unwrap();

            // Should use exactly /tmp/my_traces, ignoring output_dir and subdirs
            assert_eq!(path.parent().unwrap(), PathBuf::from("/tmp/my_traces"));

            // Clean up
            std::env::remove_var("PERFETTO_TRACE_DIR");
        }
    }

    #[test]
    fn test_comprehensive_extended_example() {
        // Test all the new extended features together
        let path = TraceFilenameBuilder::new()
            .prepend("benchmark")
            .name("sha256")
            .iteration(1)
            .machine_name("workstation")
            .thread_mode("async")
            .thread_count(16)
            .config("release")
            .run_id("exp001")
            .add("custom", "value")
            .add_option("opt", Some("present"))
            .add_option("missing", None::<String>)
            .timestamp()
            .git_info()
            .platform()
            .hostname()
            .output_dir("./traces")
            .subdir("extended")
            .subdir("tests")
            .build()
            .unwrap();

        let filename = path.file_name().unwrap().to_string_lossy();
        let full_path = path.to_string_lossy();

        // Check filename components
        assert!(filename.contains("benchmark"));
        assert!(filename.contains("value"));
        assert!(filename.contains("present"));
        assert!(!filename.contains("missing"));
        assert!(filename.contains("sha256"));
        assert!(filename.contains("iter1"));
        assert!(filename.contains("workstation"));
        assert!(filename.ends_with(".perfetto-trace"));

        // Check directory structure
        assert!(full_path.contains("traces"));
        assert!(full_path.contains("extended"));
        assert!(full_path.contains("tests"));
    }
}

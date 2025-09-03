// Integration tests to ensure both old and new APIs work correctly
use rusty_fork::rusty_fork_test;
use tracing_profile::init_tracing;

#[cfg(feature = "gen_filename")]
use tracing_profile::{init_tracing_with_builder, TraceFilenameBuilder};

#[cfg(feature = "perfetto")]
use std::path::Path;

use tracing_profile::test_utils::PerfettoTestDir;

// Since tracing_subscriber::registry() is a global singleton, we need to run the tests in separate processes.
rusty_fork_test! {
    #[test]
    fn test_old_init_tracing_works() {
        // This test ensures the original API is completely unchanged
        let _test_dir = PerfettoTestDir::new();
        let result = init_tracing();
        assert!(result.is_ok(), "Original init_tracing should still work: {:?}", result.err());
    }

    #[cfg(feature = "gen_filename")]
    #[test]
    fn test_new_init_tracing_with_builder_works() {
        // This test ensures the new API works correctly
        let _test_dir = PerfettoTestDir::new();
        let builder = TraceFilenameBuilder::new()
            .name("test_app")
            .timestamp()
            .platform()
            .hostname();

        let result = init_tracing_with_builder(builder);
        assert!(result.is_ok(), "New init_tracing_with_builder should work: {:?}", result.err());
    }

    #[cfg(feature = "perfetto")]
    #[test]
    fn test_last_trace_path_file_created() {
        // Test that .last_perfetto_trace_path file is created by both paths
        let _test_dir = PerfettoTestDir::new();

        // Clean up any existing file
        let _ = std::fs::remove_file(".last_perfetto_trace_path");

        // Test with builder
        let builder = TraceFilenameBuilder::new()
            .name("integration_test")
            .timestamp()
            .platform();

        let _guard = init_tracing_with_builder(builder).expect("Should initialize successfully");

        // Check that the file was created
        assert!(
            Path::new(".last_perfetto_trace_path").exists(),
            ".last_perfetto_trace_path should be created by new API"
        );
    }

    #[cfg(feature = "perfetto")]
    #[test]
    fn test_environment_variable_override_still_works() {
        // Test that PERFETTO_TRACE_FILE_PATH still works with builder
        let _test_dir = PerfettoTestDir::new();
        std::env::set_var("PERFETTO_TRACE_FILE_PATH", "/tmp/test_override.perfetto-trace");

        let builder = TraceFilenameBuilder::new()
            .name("test_override");

        let result = init_tracing_with_builder(builder);
        assert!(result.is_ok(), "Environment override should still work with builder: {:?}", result.err());

        // We know the override works from the env_override_test.rs - the main goal here
        // is to ensure the integration doesn't break when environment variables are set.

        // Clean up the extra environment variable we set for this test
        std::env::remove_var("PERFETTO_TRACE_FILE_PATH");
    }
}

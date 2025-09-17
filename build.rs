// Copyright 2024-2025 Irreducible Inc.

use git2::{Repository, StatusOptions};
use std::env;

fn main() {
    // Only capture build-time metadata if the gen_filename feature is enabled
    if env::var("CARGO_FEATURE_GEN_FILENAME").is_ok() {
        // Capture target platform information (what we're compiling FOR)
        capture_platform_info();

        // Capture git repository information
        capture_git_info();
    }
}

fn capture_platform_info() {
    // These are compile-time constants that represent the target platform

    // Use the TARGET environment variable provided by Cargo during build
    let target = env::var("TARGET").unwrap_or_else(|_| "unknown".to_string());

    // Extract architecture from target triple (first part)
    let arch = target.split('-').next().unwrap_or("unknown");

    println!("cargo:rustc-env=BUILD_TARGET_ARCH={arch}");
    println!("cargo:rustc-env=BUILD_TARGET_TRIPLE={target}");

    // Also capture the compile-time OS and family constants
    println!("cargo:rustc-env=BUILD_TARGET_OS={}", std::env::consts::OS);
    println!(
        "cargo:rustc-env=BUILD_TARGET_FAMILY={}",
        std::env::consts::FAMILY
    );
}

fn capture_git_info() {
    // Check if we're being built as a dependency
    let is_primary = env::var("CARGO_PRIMARY_PACKAGE").is_ok();

    // Best effort: Try to get git info from the consuming project when built as a dependency
    // Note: This relies on $PWD which is typically set by the shell, but may not always be available
    // or accurate (e.g., in some CI environments or when cargo is invoked programmatically)
    let repo_path = if !is_primary {
        // When built as a dependency, $PWD *usually* contains the directory where cargo was invoked
        // This is often the consuming project's directory, but this is not guaranteed
        if let Ok(pwd) = env::var("PWD") {
            println!("cargo:warning=Best effort: Attempting to use git info from: {}", pwd);
            pwd
        } else {
            // Fallback to our own directory if $PWD is not available
            println!("cargo:warning=PWD not available, using library's own git info");
            ".".to_string()
        }
    } else {
        // When building as primary package, use our own git info
        ".".to_string()
    };

    // Ensure we rebuild if git state changes
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/index");

    // Try to discover the repository
    let repo = match Repository::discover(&repo_path) {
        Ok(repo) => repo,
        Err(_) => {
            // No git repo found - set defaults
            println!("cargo:rustc-env=BUILD_GIT_BRANCH=unknown");
            println!("cargo:rustc-env=BUILD_GIT_COMMIT_SHORT=unknown");
            println!("cargo:rustc-env=BUILD_GIT_DIRTY=false");
            println!("cargo:rustc-env=BUILD_GIT_COMMIT_MESSAGE=");
            println!("cargo:rustc-env=BUILD_GIT_COMMIT_AUTHOR=");
            println!("cargo:rustc-env=BUILD_GIT_COMMIT_TIME=");
            return;
        }
    };

    // Get branch name
    let branch = repo
        .head()
        .ok()
        .and_then(|head| head.shorthand().map(|s| s.to_string()))
        .unwrap_or_else(|| "unknown".to_string());

    // Monitor the actual branch ref if we got one
    if !branch.starts_with("HEAD") && branch != "unknown" {
        println!("cargo:rerun-if-changed=.git/refs/heads/{branch}");
    }

    // Get commit information
    let (commit_short, commit_message, commit_author, commit_time) = repo
        .head()
        .ok()
        .and_then(|head| head.peel_to_commit().ok())
        .map(|commit| {
            let commit_short = commit.id().to_string()[..7].to_string();
            let commit_message = commit
                .message()
                .map(|m| m.lines().next().unwrap_or("").trim().to_string())
                .unwrap_or_default();
            let commit_author = commit
                .author()
                .name()
                .map(|s| s.to_string())
                .unwrap_or_default();

            // Format commit time as RFC3339 (ISO-8601)
            // For simplicity, we'll just leave it empty for now
            // (it's optional and only used in Perfetto metadata)
            let commit_time = String::new();

            (commit_short, commit_message, commit_author, commit_time)
        })
        .unwrap_or_else(|| {
            (
                "unknown".to_string(),
                String::new(),
                String::new(),
                String::new(),
            )
        });

    // Check if working tree is clean
    let is_clean = {
        let mut opts = StatusOptions::new();
        opts.include_untracked(true)
            .recurse_untracked_dirs(true)
            .include_ignored(false);

        repo.statuses(Some(&mut opts))
            .map(|statuses| statuses.is_empty())
            .unwrap_or(true)
    };

    // Set environment variables that will be available at compile time
    println!("cargo:rustc-env=BUILD_GIT_BRANCH={branch}");
    println!("cargo:rustc-env=BUILD_GIT_COMMIT_SHORT={commit_short}");
    println!("cargo:rustc-env=BUILD_GIT_DIRTY={}", !is_clean);
    println!("cargo:rustc-env=BUILD_GIT_COMMIT_MESSAGE={commit_message}");
    println!("cargo:rustc-env=BUILD_GIT_COMMIT_AUTHOR={commit_author}");
    println!("cargo:rustc-env=BUILD_GIT_COMMIT_TIME={commit_time}");
}

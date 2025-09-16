// Copyright 2024-2025 Irreducible Inc.

//https://android.googlesource.com/platform/external/perfetto/+/refs/tags/android-14.0.0_r50/examples/sdk/
//https://perfetto.dev/docs/instrumentation/tracing-sdk
fn main() {
    // Building cpp will fail on targets other than macOS, Linux and Android so we perform this check
    // to produce a meaningful error message.
    if !matches!(
        std::env::var("CARGO_CFG_TARGET_OS").as_deref(),
        Ok("linux") | Ok("macos") | Ok("android")
    ) {
        panic!("Perfetto tracing works only on Linux, macOS, and Android");
    }

    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rerun-if-changed=cpp");

    cc::Build::new()
        .cpp(true)
        .opt_level(
            std::env::var("CARGO_OPT_LEVEL")
                .unwrap_or_else(|_| "2".to_string())
                .parse()
                .unwrap(),
        )
        .flag("-std=c++20")
        .file("cpp/wrapper.cc")
        .file("cpp/trace_categories.cc")
        // TODO: extract perfetto to a static library
        .file("cpp/perfetto/sdk/perfetto.cc")
        .include("cpp")
        .include("cpp/perfetto/sdk")
        .compile("perfettoWrapper");
}

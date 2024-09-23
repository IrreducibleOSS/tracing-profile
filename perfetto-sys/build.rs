//https://android.googlesource.com/platform/external/perfetto/+/refs/tags/android-14.0.0_r50/examples/sdk/
//https://perfetto.dev/docs/instrumentation/tracing-sdk
fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rerun-if-changed=cpp");

    cc::Build::new()
        .cpp(true)
        .opt_level(std::env::var("CARGO_OPT_LEVEL").unwrap_or_else(|_| "2".to_string()).parse().unwrap())
        .flag("-std=c++20")
        .file("cpp/wrapper.cc")
        .file("cpp/trace_categories.cc")
        // TODO: extract perfetto to a static library
        .file("cpp/perfetto/sdk/perfetto.cc")
        .include("cpp")
        .include("cpp/perfetto/sdk")
        .compile("perfettoWrapper");
}

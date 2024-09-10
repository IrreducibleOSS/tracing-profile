use std::{env, fs::create_dir_all, path::Path, process::Command};

use cmake::Config;
use convert_case::{Case, Casing};
use serde::Deserialize;
use std::io::Write;

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum ValueType {
    Int,
    Float,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum InterfaceEntity {
    Event {
        category: String,
    },
    Counter {
        category: String,
        unit: Option<String>,
        value_type: ValueType,
    },
}

#[derive(Deserialize)]
struct Category {
    name: String,
    description: String,
}

#[derive(Deserialize)]
struct InterfaceData {
    categories: Vec<Category>,
    interface: Vec<InterfaceEntity>,
}

fn parse_interface() -> InterfaceData {
    let Ok(file_name) = env::var("PERFETTO_INTERFACE_FILE") else {
        // No file given, use the default interface
        return InterfaceData {
            categories: vec![Category {
                name: "default".to_string(),
                description: "Default category".to_string(),
            }],
            interface: vec![
                InterfaceEntity::Event {
                    category: "default".to_string(),
                },
                InterfaceEntity::Counter {
                    category: "default".to_string(),
                    unit: None,
                    value_type: ValueType::Int,
                },
            ],
        };
    };

    let file = std::fs::File::open(file_name).expect("cannot open interface file");
    serde_json::from_reader(file).expect("failed to deserialize interface")
}

fn cpp_category_literal_var(category: &str) -> String {
    format!("{}_TRACE_CATEGORY", category.to_case(Case::UpperSnake))
}

fn rs_category_enum_member(category: &str) -> String {
    category.to_case(Case::UpperCamel)
}

fn generate_trace_categories_h(dir: &Path, categories: &[Category]) {
    let categories_file = Path::new(&dir).join("trace_categories.h");
    let mut writer =
        std::fs::File::create(categories_file).expect("cannot create 'trace_categories.h'");

    let category_literals = categories
        .iter()
        .map(|category| {
            format!(
                r#"constexpr char {}[] = "{}";"#,
                cpp_category_literal_var(&category.name),
                category.name
            )
        })
        .collect::<Vec<String>>()
        .join("\n");

    let category_defs = categories
        .iter()
        .map(|category| {
            format!(
                r#"    perfetto::Category({}).SetDescription("{}")"#,
                cpp_category_literal_var(&category.name),
                category.description
            )
        })
        .collect::<Vec<String>>()
        .join(",\n");

    write!(
        writer,
        r#"#pragma once

#include "perfetto/sdk/perfetto.h"

{}

PERFETTO_DEFINE_CATEGORIES(
{}
);
"#,
        category_literals, category_defs
    )
    .unwrap();
}

fn generate_interface_wrappers(cpp_dir: &Path, rs_dir: &Path, interface: &InterfaceData) {
    let mut cpp_method_decls = Vec::new();
    let mut cpp_method_defs = Vec::new();

    let mut rs_extern_method_delcs = Vec::new();

    let mut rs_event_categories = Vec::new();
    let mut rs_create_event_match_cases = Vec::new();
    let mut rs_destroy_event_match_cases = Vec::new();
    let mut rs_event_category_from_str_match_cases = Vec::new();

    let mut rs_counter_categories = Vec::new();
    let mut rs_update_counter_match_cases = Vec::new();
    let mut rs_counter_category_from_str_match_cases = Vec::new();

    for entity in interface.interface.iter() {
        match entity {
            InterfaceEntity::Event { category } => {
                cpp_method_decls.push(format!("void create_event_{category}(const char* name);"));
                cpp_method_decls.push(format!("void destroy_event_{category}();"));
                cpp_method_defs.push(format!(
                    r#"
void create_event_{0}(const char* event_label) {{
    TRACE_EVENT_BEGIN({1}, perfetto::DynamicString{{event_label}});
}}

void destroy_event_{0}() {{
    TRACE_EVENT_END({1});
}}
"#,
                    category,
                    cpp_category_literal_var(category)
                ));

                rs_extern_method_delcs
                    .push(format!("fn create_event_{category}(name: *const c_char);"));
                rs_extern_method_delcs.push(format!("fn destroy_event_{category}();"));

                let category_enum_member = rs_category_enum_member(&category);
                rs_event_categories.push(category_enum_member.clone());

                rs_create_event_match_cases.push(format!(
                    r#"
        EventCategory::{category_enum_member} => {{ create_event_{category}(name) }}
"#
                ));
                rs_destroy_event_match_cases.push(format!(
                    r#"
        EventCategory::{category_enum_member} => {{ destroy_event_{category}() }}
"#
                ));
                rs_event_category_from_str_match_cases.push(format!(
                    r#"
        "{category}" => Ok(EventCategory::{category_enum_member}),"#,
                ));
            }
            InterfaceEntity::Counter {
                category,
                unit,
                value_type,
            } => {
                let (cpp_counter_type, rs_counter_type) = match value_type {
                    ValueType::Int => ("int32_t", "i32"),
                    ValueType::Float => ("float", "f32"),
                };
                cpp_method_decls.push(format!(
                    "void counter_{category}(const char* label, {cpp_counter_type} value);"
                ));

                let label = match unit {
                    Some(unit) => format!("perfetto::CounterTrack(label, \"{unit}\")"),
                    None => "label".to_string(),
                };
                cpp_method_defs.push(format!(
                    r#"
void counter_{0}(const char* label, {2} value) {{
	TRACE_COUNTER({1}, {3}, value);
}}
"#,
                    category,
                    cpp_category_literal_var(category),
                    cpp_counter_type,
                    label
                ));

                rs_extern_method_delcs.push(format!(
                    "fn counter_{category}(label: *const c_char, value: {rs_counter_type});"
                ));

                let category_enum_member = rs_category_enum_member(&category);
                rs_counter_categories.push(category_enum_member.clone());

                rs_update_counter_match_cases.push(format!(r#"
        CounterCategory::{category_enum_member} => {{ counter_{category}(label, value.try_into().expect("invalid value type")) }}"#));

                rs_counter_category_from_str_match_cases.push(format!(
                    r#"
        "{category}" => Ok(CounterCategory::{category_enum_member}),"#
                ));
            }
        }
    }

    // generate 'interface_wrapper.h'
    let wrappers_header = Path::new(&cpp_dir).join("interface_wrapper.h");
    let mut writer =
        std::fs::File::create(wrappers_header).expect("cannot create 'interface_wrapper.h'");
    write!(
        writer,
        r#"#pragma once

#include <cstdint>

extern "C" {{
{}
}}
"#,
        cpp_method_decls.join("\n")
    )
    .unwrap();

    // generate 'interface_wrapper.cc'
    let wrappers_source = Path::new(&cpp_dir).join("interface_wrapper.cc");
    let mut writer =
        std::fs::File::create(wrappers_source).expect("cannot create 'interface_wrapper.cc'");
    write!(
        writer,
        r#"#include "interface_wrapper.h"

#include "trace_categories.h"

{}

"#,
        cpp_method_defs.join("\n")
    )
    .unwrap();

    // generate 'interface.rs'
    let rs_interface = Path::new(&rs_dir).join("interface.rs");
    let mut writer = std::fs::File::create(rs_interface).expect("cannot create 'interface.rs'");
    write!(writer,
r#"
extern "C" {{
{}
}}

/// Event categories
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum EventCategory {{
{}
}}

impl TryFrom<&str> for EventCategory {{
    type Error = &'static str;

    fn try_from(value: &str) -> Result<Self, Self::Error> {{
        match value {{
{}
            _ => Err("unknown event category"),
        }}
    }}
}}

/// Counter categories
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum CounterCategory {{
{}
}}

impl TryFrom<&str> for CounterCategory {{
    type Error = &'static str;

    fn try_from(value: &str) -> Result<Self, Self::Error> {{
        match value {{
{}
            _ => Err("unknown counter category"),
        }}
    }}
}}

/// Create a new event
unsafe fn create_event(category: EventCategory, name: *const c_char) {{
    match category {{
{}
    }}
}}

/// Destroy event
unsafe fn destroy_event(category: EventCategory) {{
    match category {{
{}
    }}
}}

/// Update a counter value
unsafe fn update_counter_impl(category: CounterCategory, label: *const c_char, value: CounterValue) {{
    match category {{
{}
    }}
}}

"#,
rs_extern_method_delcs.join("\n"),
rs_event_categories.join(",\n"),
rs_event_category_from_str_match_cases.join(",\n"),
rs_counter_categories.join(",\n"),
rs_counter_category_from_str_match_cases.join(",\n"),
rs_create_event_match_cases.join(""),
rs_destroy_event_match_cases.join(""),
rs_update_counter_match_cases.join(""),
).unwrap();
}

fn generate_interface() {
    let interface = parse_interface();

    let out_dir = env::var_os("OUT_DIR").unwrap();
    let generated_dir = Path::new(&out_dir).join("generated");
    let cpp_dir = generated_dir.join("cpp");
    create_dir_all(&cpp_dir).expect("cannot create 'cpp' directory");
    let rs_dir = generated_dir.join("rs");
    create_dir_all(&rs_dir).expect("cannot create 'rs' directory");

    generate_trace_categories_h(&cpp_dir, &interface.categories);
    generate_interface_wrappers(&cpp_dir, &rs_dir, &interface);
}

//https://android.googlesource.com/platform/external/perfetto/+/refs/tags/android-14.0.0_r50/examples/sdk/
//https://perfetto.dev/docs/instrumentation/tracing-sdk
fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let curr_dir = Path::new(&manifest_dir);
    Command::new("git")
        .arg("submodule")
        .arg("update")
        .arg("--init")
        .current_dir(curr_dir)
        .status()
        .unwrap();

    // generate the interface files
    generate_interface();

    let build_dir = Config::new("cpp").no_build_target(true).build();
    // without this, the cmake crate attempts to install the static library. i don't want to do that.
    let _output = std::process::Command::new("cmake")
        .args(["--build", build_dir.to_str().unwrap()])
        .status()
        .expect("Failed to build the CMake project");

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/lib.rs");
    println!("cargo:rerun-if-changed=cpp/wrapper.cc");
    println!("cargo:rerun-if-changed=cpp/wrapper.h");
    println!("cargo:rerun-if-changed=cpp/trace_categories.cc");
    println!("cargo:rerun-if-changed=cpp/trace_categories.h");
    println!("cargo:rerun-if-changed=Cargo.lock");
    println!(
        "cargo:rustc-link-search=native={}/build",
        build_dir.display()
    );
    println!("cargo:rustc-link-lib=dylib=stdc++");
    println!("cargo:rustc-link-lib=static=perfetto");
    println!("cargo:rustc-link-lib=static=perfettoWrapper");
}

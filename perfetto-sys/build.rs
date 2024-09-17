use std::{env, fs::create_dir_all, path::Path};

use convert_case::{Case, Casing};
use serde::Deserialize;
use std::io::Write;
use std::path::PathBuf;

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

fn generate_trace_categories_h(paths: &GeneratedMeta, categories: &[Category]) {
    let mut writer = std::fs::File::create(&paths.categories_header_path)
        .expect("cannot create 'trace_categories.h'");

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

fn generate_interface_wrappers(paths: &GeneratedMeta, interface: &InterfaceData) {
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
    let mut writer = std::fs::File::create(&paths.wrappers_header_path)
        .expect("cannot create 'interface_wrapper.h'");
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
    let mut writer =
        std::fs::File::create(&paths.wrappers_source_path).expect("cannot create 'interface_wrapper.cc'");
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
    let mut writer = std::fs::File::create(&paths.rs_interface_path).expect("cannot create 'interface.rs'");
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

fn generate_interface() -> GeneratedMeta {
    let interface = parse_interface();

    let out_dir = env::var_os("OUT_DIR").unwrap();
    let generated_dir = Path::new(&out_dir).join("generated");
    let cpp_dir = generated_dir.join("cpp");
    create_dir_all(&cpp_dir).expect("cannot create 'cpp' directory");
    let rs_dir = generated_dir.join("rs");
    create_dir_all(&rs_dir).expect("cannot create 'rs' directory");

    let paths = GeneratedMeta {
        cpp_header_dir: cpp_dir.clone(),
        categories_header_path: Path::new(&cpp_dir).join("trace_categories.h"),
        wrappers_header_path: Path::new(&cpp_dir).join("interface_wrapper.h"),
        wrappers_source_path: Path::new(&cpp_dir).join("interface_wrapper.cc"),
        rs_interface_path: Path::new(&rs_dir).join("interface.rs"),
    };
    generate_trace_categories_h(&paths, &interface.categories);
    generate_interface_wrappers(&paths, &interface);

    paths
}

struct GeneratedMeta {
    cpp_header_dir: PathBuf,
    categories_header_path: PathBuf,
    wrappers_header_path: PathBuf,
    wrappers_source_path: PathBuf,
    rs_interface_path: PathBuf,
}

//https://android.googlesource.com/platform/external/perfetto/+/refs/tags/android-14.0.0_r50/examples/sdk/
//https://perfetto.dev/docs/instrumentation/tracing-sdk
fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rerun-if-changed=cpp");
    if let Ok(file_name) = env::var("PERFETTO_INTERFACE_FILE") {
        println!("cargo::rerun-if-changed={}", file_name);
    }
    println!("cargo::rerun-if-env-changed=PERFETTO_INTERFACE_FILE");

    let gen_paths = generate_interface();

    cc::Build::new()
        .cpp(true)
        .file("cpp/wrapper.cc")
        .file("cpp/trace_categories.cc")
        .file("cpp/perfetto/sdk/perfetto.cc")
        .file(gen_paths.wrappers_source_path)
        .include("cpp")
        .include("cpp/perfetto/sdk")
        .include(gen_paths.cpp_header_dir)
        .compile("perfettoWrapper");
}

cmake_minimum_required(VERSION 3.13)
project(PerfettoWrapper VERSION 1.0.0 DESCRIPTION "c-style interface for Perfetto")
find_package(Threads)
 
# Define a static library for Perfetto.
include_directories(perfetto/sdk)
add_library(perfetto STATIC perfetto/sdk/perfetto.cc)

# Define a wrapper library that uses the Perfetto library.
set(GENERATED_DIR "$ENV{OUT_DIR}/generated/cpp")
add_library(perfettoWrapper STATIC 
    wrapper.cc 
    trace_categories.cc
    "${GENERATED_DIR}/interface_wrapper.cc")
target_include_directories(perfettoWrapper PRIVATE "${CURRENT_SOURCE_DIR}" "${GENERATED_DIR}")
set_target_properties(perfettoWrapper PROPERTIES
    VERSION ${PROJECT_VERSION}
    SOVERSION ${PROJECT_VERSION_MAJOR}
    PUBLIC_HEADER wrapper.h)

target_link_libraries(perfettoWrapper PRIVATE perfetto ${CMAKE_THREAD_LIBS_INIT})
// Copyright 2024-2025 Irreducible Inc.

#pragma once

#include <cstdint>
#include <cstddef>

/// Argument types for the PerfettoEventArg struct.
enum class ArgType : uint8_t {
    FlowID,
    StringKeyValue,
    F64KeyValue,
    I64KeyValue,
    U64KeyValue,
    BoolKeyValue,
};

/// Event types for the PerfettoEventArg struct.
enum class EventType {
	Span,
	Instant,
};

/// Key-value pair with string key for the PerfettoEventArg struct.
template <typename T>
struct KeyValue {
    const char* key;
    T value;
};

struct PerfettoEventArg {
    union {
        const uint64_t u64;
        KeyValue<const char*> string_key_value;
        KeyValue<double> f64_key_value;
        KeyValue<int64_t> i64_key_value;
        KeyValue<uint64_t> u64_key_value;
        KeyValue<bool> bool_key_value;
    } data;
    ArgType type;
};

extern "C" {
/// @brief Initialize the Perfetto tracing system.
/// @param backend_type is the type of backend to use. See `perfetto::BackendType` for possible values.
/// @param output_file is the path to the file to write the trace to. Must not be null if `backend_type` is not "System".
/// @param buffer_size_kb is the size of the buffer to use in kilobytes for non-system backend.
void *init_perfetto(uint32_t backend_type, const char* output_file, size_t buffer_size_kb);

/// @brief Deinitialize the Perfetto tracing system.
/// @param guard is the pointer returned by `init_perfetto`, must not be null.
/// This function will free the resources allocated by `init_perfetto` and cannot be called twice for the same `guard`.
void deinit_perfetto(void *guard);

/// @brief Start a new tracking event.
/// @param event_type Event type.
/// @param category Event category. If null, the default category will be used.
/// @param name Event name. Must not be null.
/// @param track_id Track ID for the event. If null, no explicit track ID will be used.
/// @param args Information about tracking, flow and additional fields.
/// @param arg_count Number of elements in `args`.
void create_event(EventType event_type, const char* category, const char* name, const uint64_t* track_id, const PerfettoEventArg* args, size_t arg_count);

/// @brief End the most recent tracking event.
/// @param category Event category. If null, the default category will be used.
/// @param track_id Track ID for the event. If null, no explicit track ID will be used. This value must correspond to the track ID used in `create_event`.
void destroy_event(const char* category, const uint64_t* track_id);

/// @brief  Update a counter with an unsigned 64-bit integer value.
/// @param category Counter category. If null, the default category will be used.
/// @param name Counter name. Must not be null.
/// @param unit Unit of the counter. If null, no unit will be used.
/// @param is_incremental If counter is incremental.
/// @param value Value of the counter.
void update_counter_u64(const char* category, const char* name, const char* unit, bool is_incremental, uint64_t value);

/// @brief  Update a counter with an 64-bit floating point value.
/// @param category Counter category. If null, the default category will be used.
/// @param name Counter name. Must not be null.
/// @param unit Unit of the counter. If null, no unit will be used.
/// @param is_incremental If counter is incremental.
/// @param value Value of the counter.
void update_counter_f64(const char* category, const char* name, const char* unit, bool is_incremental, double value);
}

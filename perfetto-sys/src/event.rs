use std::{ffi::{c_char, CString}, ptr::null, thread::{self, ThreadId}};
use std::sync::{Mutex, OnceLock};
use std::collections::HashMap;
 
// Get stable pointer for `key`
fn get_key_ptr(key: &'static str) -> *const c_char {
    static KEY_POOL: OnceLock<Mutex<HashMap<&'static str, CString>>> = OnceLock::new();
    let map = KEY_POOL.get_or_init(|| Mutex::new(HashMap::new()));
    let mut guard = map.lock().unwrap();
    guard
        .entry(key)
        .or_insert_with(|| CString::new(key).expect("invalid key string"))
        .as_ptr()
}


#[repr(u8)]
enum ArgType {
    FlowID = 0,
    StringKeyValue,
    F64KeyValue,
    I64KeyValue,
    U64KeyValue,
    BoolKeyValue,
}

#[repr(u8)]
enum EventType {
    Span,
    Instant,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct KeyValue<T> {
    key: *const c_char,
    value: T,
}

#[repr(C)]
union ArgValue {
    u64: u64,
    string_key_value: KeyValue<*const c_char>,
    f64_key_value: KeyValue<f64>,
    i64_key_value: KeyValue<i64>,
    u64_key_value: KeyValue<u64>,
    bool_key_value: KeyValue<bool>,
}

#[repr(C)]
struct PerfettoArg {
    data: ArgValue,
    arg_type: ArgType,
}

extern "C" {
    fn create_event(event_type: EventType, category: *const c_char, name: *const c_char, track_id: *const u64, args: *const PerfettoArg, arg_count: usize);
    fn destroy_event(category: *const c_char, track_id: *const u64);
}

/// Represents a tracing event data.
pub struct EventData {
    /// Name of the event.
    name: CString,
    /// Category of the event. If None the default will be used
    category: Option<CString>,
    /// Track id of the event. If None the current thread track will be used.
    track_id: Option<u64>,
    /// Information about custom fields and flow id
    args: Vec<PerfettoArg>,
    /// Storage for the strings in the args
    strings_storage: Vec<CString>,
}

impl EventData {
    pub fn new(name: &str) -> Self {
        Self {
            category: None,
            track_id: None,
            name: CString::new(name).unwrap(),
            strings_storage: Vec::new(),
            args: Vec::new(),
        }
    }

    pub fn set_category(&mut self, category: &str) {
        self.category = Some(CString::new(category).expect("category is not a valid string"));
    }

    pub fn set_track_id(&mut self, track_id: u64) {
        self.track_id = Some(track_id);
    }

    pub fn set_flow_id(&mut self, flow_id: u64) {
        self.args.push(PerfettoArg {
            data: ArgValue { u64: flow_id },
            arg_type: ArgType::FlowID,
        });
    }

    pub fn add_u64_field(&mut self, key: &'static str, value: u64) {
        let key_ptr = get_key_ptr(key);
        self.args.push(PerfettoArg {
            data: ArgValue {  u64_key_value: KeyValue { key: key_ptr, value } },
            arg_type: ArgType::U64KeyValue,
        });
    }

    pub fn add_i64_field(&mut self, key: &'static str, value: i64) {
        let key_ptr = get_key_ptr(key);
        self.args.push(PerfettoArg {
            data: ArgValue { i64_key_value: KeyValue { key: key_ptr, value } },
            arg_type: ArgType::I64KeyValue,
        });
    }

    pub fn add_f64_field(&mut self, key: &'static str, value: f64) {
        let key_ptr = get_key_ptr(key);
        self.args.push(PerfettoArg {
            data: ArgValue { f64_key_value: KeyValue { key: key_ptr, value } },
            arg_type: ArgType::F64KeyValue,
        });
    }

    pub fn add_bool_field(&mut self, key: &'static str, value: bool) {
        let key_ptr = get_key_ptr(key);
        self.args.push(PerfettoArg {
            data: ArgValue { bool_key_value: KeyValue { key: key_ptr, value } },
            arg_type: ArgType::BoolKeyValue,
        });
    }

    pub fn add_string_arg(&mut self, key: &'static str, value: &str) {
        let key_ptr = get_key_ptr(key);
        let value = CString::new(value).expect("value is invalid string");
        self.args.push(PerfettoArg {
            data: ArgValue { string_key_value: KeyValue { key: key_ptr, value: value.as_ptr() } },
            arg_type: ArgType::StringKeyValue,
        });
        self.strings_storage.push(value);
    }
}

/// Safety: raw pointers in EventData.args remain valid because field key strings are stored globally (static lifetime),
/// and any value strings are stored in this EventData's strings_storage.
unsafe impl Send for EventData {}
unsafe impl Sync for EventData {}

#[derive(Debug)]
enum Track {
    CurrentThread(ThreadId),
    Custom(u64),
}

/// Represents a tracing span. Will exist until the struct is dropped.
#[derive(Debug)]
pub struct TraceEvent {
    track: Track,
    category: Option<CString>,
}

impl TraceEvent {
    pub fn new(event_data: EventData) -> Self {
        unsafe { create_event(
            EventType::Span,
            event_data.category.as_ref().map(|s| s.as_ptr()).unwrap_or(null()), 
            event_data.name.as_ptr(),
            event_data.track_id.as_ref().map(|id| id as *const u64).unwrap_or(null()),
            event_data.args.as_ptr(), 
            event_data.args.len()) };
        
        let track = match event_data.track_id {
            Some(track_id) => Track::Custom(track_id),
            None => Track::CurrentThread(thread::current().id()),
        };
        Self {
            track,
            category: event_data.category,
        }
    }
}

impl Drop for TraceEvent {
    fn drop(&mut self) {
        let track_id = match &self.track {
            Track::CurrentThread(thread_id) => {
                assert!(*thread_id == thread::current().id());
                null()
            }
            Track::Custom(track_id) => {
                track_id as *const u64
            },
        };

        unsafe { destroy_event(self.category.as_ref().map(|s| s.as_ptr()).unwrap_or(null()), track_id) };
    }
}

/// Emit the given `EventData` as a Perfetto instant event with all metadata.
pub fn create_instant_event(event_data: EventData) {
    unsafe {
        create_event(
            EventType::Instant,
            event_data.category.as_ref().map(|s| s.as_ptr()).unwrap_or(null()),
            event_data.name.as_ptr(),
            event_data.track_id.as_ref().map(|id| id as *const u64).unwrap_or(null()),
            event_data.args.as_ptr(),
            event_data.args.len(),
        );
    }
}

include!(concat!(env!("OUT_DIR"), "/generated/rs/interface.rs"));

use core::ffi::c_char;
use std::{
    ffi::{c_void, CString},
    thread::{self, ThreadId},
    time::Duration,
};

extern "C" {
    fn init_perfetto(backend: u32, output_path: *const i8, buffer_size: usize) -> *mut c_void;
    fn deinit_perfetto(guard: *mut c_void);
}

// Safety: the pointers here are heap allocated and not shared. should be ok to send them to other threads
unsafe impl Send for PerfettoGuard {}
unsafe impl Sync for PerfettoGuard {}

/// Create only one of these per tracing session. It should live for the duration of the program.
#[derive(Debug)]
pub struct PerfettoGuard {
    ptr: *mut c_void,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    /// included for completeness. will likely have no effect.
    Unknown = 0,
    /// creates a tracing service on a dedicated thread.
    InProcess = 1,
    /// requires the traced service to be running separately. is used to collect a fused trace
    System = 2,
    /// included for completeness. will likely have no effect.
    Custom = 4,
}

impl PerfettoGuard {
    /// Initializes system wide tracing. Will block the current thread until it establishes a connection to the perfetto tracing service.
    /// `output_path` is the path to the trace file for in-process mode. If `None`, "tracing.perfetto-trace" will be used.
    /// `buffer_size_kb` is the size of the buffer for in-process mode in kilobytes. 
    pub fn new(backend: Backend, output_path: Option<&str>, buffer_size_kb: usize) -> Self {
        let output_path = output_path.map(|s| CString::new(s).unwrap());
        let output_path_ptr = output_path
            .as_ref()
            .map(|s| s.as_ptr())
            .unwrap_or(std::ptr::null());
        let ptr = unsafe { init_perfetto(backend as u32, output_path_ptr, buffer_size_kb) };
        Self { ptr }
    }
}

impl Drop for PerfettoGuard {
    fn drop(&mut self) {
        // in wrapper.cc there's a 5 second flush interval. want to ensure all logs are flushed before stopping perfetto.
        std::thread::sleep(Duration::from_millis(5500));
        unsafe { deinit_perfetto(self.ptr) }
    }
}

/// Represents a tracing span. Will exist until the struct is dropped.
#[derive(Debug)]
pub struct TraceEvent {
    thread_id: ThreadId,
    category: EventCategory,
}

impl TraceEvent {
    pub fn new(label: &str, category: EventCategory) -> Self {
        let c_str = CString::new(label).unwrap();
        let c_ptr = c_str.as_ptr() as *const c_char;
        unsafe { create_event(category, c_ptr) };
        Self {
            thread_id: thread::current().id(),
            category,
        }
    }
}

impl Drop for TraceEvent {
    fn drop(&mut self) {
        if self.thread_id == thread::current().id() {
            unsafe { destroy_event(self.category) };
        } else {
            panic!("TraceEvent dropped on a different thread than it was created!");
        }
    }
}

pub enum CounterValue {
    Int32(i32),
    Float(f32),
}

impl TryFrom<CounterValue> for f32 {
    type Error = &'static str;

    fn try_from(value: CounterValue) -> Result<Self, Self::Error> {
        match value {
            CounterValue::Int32(_) => Err("invalid type, expected float"),
            CounterValue::Float(f) => Ok(f),
        }
    }
}

impl TryFrom<CounterValue> for i32 {
    type Error = &'static str;

    fn try_from(value: CounterValue) -> Result<Self, Self::Error> {
        match value {
            CounterValue::Int32(i) => Ok(i),
            CounterValue::Float(_) => Err("invalid type, expected i32"),
        }
    }
}

pub fn update_counter(category: CounterCategory, label: &str, value: CounterValue) {
    let c_str = CString::new(label).unwrap();
    let c_ptr = c_str.as_ptr() as *const c_char;
    unsafe {
        update_counter_impl(category, c_ptr, value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backend() {
        let _guard = PerfettoGuard::new(Backend::System, None, 10);
    }

    #[test]
    fn test_in_process() {
        let _guard = PerfettoGuard::new(Backend::InProcess, None, 10);
    }
}

// Copyright 2024-2025 Irreducible Inc.

use std::{
    ffi::{c_char, CString},
    ptr::null,
};

extern "C" {
    fn update_counter_u64(
        category: u32,
        name: *const c_char,
        unit: *const c_char,
        is_increment: bool,
        value: u64,
    );
    fn update_counter_f64(
        category: u32,
        name: *const c_char,
        unit: *const c_char,
        is_increment: bool,
        value: f64,
    );
}

/// Update the value of a counter with a 64-bit unsigned integer.
pub fn set_counter_u64(name: &str, unit: Option<&str>, is_incremental: bool, value: u64) {
    let name = CString::new(name).unwrap();
    let unit = unit.map(|s| CString::new(s).unwrap());
    unsafe {
        update_counter_u64(
            0,
            name.as_ptr(),
            unit.as_ref().map(|s| s.as_ptr()).unwrap_or(null()),
            is_incremental,
            value,
        )
    }
}

/// Update the value of a counter with a 64-bit floating point number.
pub fn set_counter_f64(name: &str, unit: Option<&str>, is_incremental: bool, value: f64) {
    let name = CString::new(name).unwrap();
    let unit = unit.map(|s| CString::new(s).unwrap());
    unsafe {
        update_counter_f64(
            0,
            name.as_ptr(),
            unit.as_ref().map(|s| s.as_ptr()).unwrap_or(null()),
            is_incremental,
            value,
        )
    }
}

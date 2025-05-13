// Copyright 2024-2025 Irreducible Inc.

use std::{borrow::Cow, fmt::Write};
use {linear_map::LinearMap, std::ops::AddAssign};

pub struct StoringFieldVisitor<'a>(pub &'a mut LinearMap<&'static str, String>);

impl tracing::field::Visit for StoringFieldVisitor<'_> {
    fn record_f64(&mut self, field: &tracing::field::Field, value: f64) {
        self.0.insert(field.name(), value.to_string());
    }

    fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
        self.0.insert(field.name(), value.to_string());
    }

    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        self.0.insert(field.name(), value.to_string());
    }

    fn record_bool(&mut self, field: &tracing::field::Field, value: bool) {
        self.0.insert(field.name(), value.to_string());
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        self.0.insert(field.name(), value.to_string());
    }

    fn record_error(
        &mut self,
        field: &tracing::field::Field,
        value: &(dyn std::error::Error + 'static),
    ) {
        self.0.insert(field.name(), value.to_string());
    }

    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        self.0.insert(field.name(), format!("{:?}", value));
    }
}

pub struct WritingFieldVisitor<'a, Writer: Write> {
    is_first: bool,
    writer: &'a mut Writer,
    separator: Cow<'static, str>,
}

impl<'a, Writer: Write> WritingFieldVisitor<'a, Writer> {
    #[allow(unused)]
    pub fn new(writer: &'a mut Writer) -> Self {
        Self::new_with_separator(writer, Cow::Borrowed(", "))
    }

    pub fn new_with_separator(writer: &'a mut Writer, separator: Cow<'static, str>) -> Self {
        Self {
            is_first: true,
            writer,
            separator,
        }
    }

    fn write_separator(&mut self) {
        if self.is_first {
            self.is_first = false;
        } else {
            self.writer
                .write_str(&self.separator)
                .expect("failed to write separator");
        }
    }
}

impl<Writer: Write> tracing::field::Visit for WritingFieldVisitor<'_, Writer> {
    fn record_f64(&mut self, field: &tracing::field::Field, value: f64) {
        self.write_separator();
        write!(self.writer, "{}: {}", field.name(), value).expect("failed to write f64");
    }

    fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
        self.write_separator();
        write!(self.writer, "{}: {}", field.name(), value).expect("failed to write i64");
    }

    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        self.write_separator();
        write!(self.writer, "{}: {}", field.name(), value).expect("failed to write u64");
    }

    fn record_bool(&mut self, field: &tracing::field::Field, value: bool) {
        self.write_separator();
        write!(self.writer, "{}: {}", field.name(), value).expect("failed to write bool");
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        self.write_separator();
        write!(self.writer, "{}: {}", field.name(), value).expect("failed to write str");
    }

    fn record_error(
        &mut self,
        field: &tracing::field::Field,
        value: &(dyn std::error::Error + 'static),
    ) {
        self.write_separator();
        write!(self.writer, "{}: {}", field.name(), value).expect("failed to write error");
    }

    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        self.write_separator();
        write!(self.writer, "{}: {:?}", field.name(), value).expect("failed to write debug");
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum CounterValue {
    Int(u64),
    Float(f64),
}

impl Default for CounterValue {
    fn default() -> Self {
        Self::Int(0)
    }
}

impl std::fmt::Display for CounterValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CounterValue::Int(val) => write!(f, "{}", val),
            CounterValue::Float(val) => write!(f, "{}", val),
        }
    }
}

impl AddAssign for CounterValue {
    fn add_assign(&mut self, rhs: Self) {
        match (self, rhs) {
            (CounterValue::Int(lhs), CounterValue::Int(rhs)) => *lhs += rhs,
            (CounterValue::Int(lhs), CounterValue::Float(rhs)) => *lhs += rhs as u64,
            (CounterValue::Float(lhs), CounterValue::Int(rhs)) => *lhs += rhs as f64,
            (CounterValue::Float(lhs), CounterValue::Float(rhs)) => *lhs += rhs,
        }
    }
}

impl AddAssign<u64> for CounterValue {
    fn add_assign(&mut self, rhs: u64) {
        match self {
            CounterValue::Int(lhs) => *lhs += rhs,
            CounterValue::Float(lhs) => *lhs += rhs as f64,
        }
    }
}

// gets the needed data out of an Event by implementing the Visit trait
#[derive(Default)]
pub struct CounterVisitor {
    pub value: Option<CounterValue>,
    pub unit: Option<String>,
    pub category: Option<String>,
    pub is_counter: bool,
    pub is_incremental: bool,
}

const COUNTER_VALUE_FIELD: &str = "value";
const IS_COUNTER_FIELD: &str = "counter";
const IS_INCREMENTAL_FIELD: &str = "incremental";
const PERFETTO_CATEGORY_FIELD: &str = "perfetto_category";
const UNIT_FIELD: &str = "unit";

impl tracing::field::Visit for CounterVisitor {
    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        if field.name() == COUNTER_VALUE_FIELD {
            self.value.replace(CounterValue::Int(value));
        }
    }

    fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
        if field.name() == COUNTER_VALUE_FIELD {
            self.value.replace(CounterValue::Int(value as _));
        }
    }

    fn record_f64(&mut self, field: &tracing::field::Field, value: f64) {
        if field.name() == COUNTER_VALUE_FIELD {
            self.value.replace(CounterValue::Float(value as _));
        }
    }

    fn record_bool(&mut self, field: &tracing::field::Field, value: bool) {
        match field.name() {
            IS_COUNTER_FIELD => self.is_counter = value,
            IS_INCREMENTAL_FIELD => self.is_incremental = value,
            _ => {}
        }
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        match field.name() {
            PERFETTO_CATEGORY_FIELD => {
                self.category.replace(value.to_string());
            }
            UNIT_FIELD => {
                self.unit.replace(value.to_string());
            }
            _ => {}
        }
    }

    fn record_debug(&mut self, _: &tracing::field::Field, _: &dyn std::fmt::Debug) {}
}

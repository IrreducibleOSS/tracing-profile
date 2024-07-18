use std::{collections::BTreeMap, fmt::Write};

pub struct StoringFieldVisitor<'a>(pub &'a mut BTreeMap<String, String>);

impl<'a> tracing::field::Visit for StoringFieldVisitor<'a> {
    fn record_f64(&mut self, field: &tracing::field::Field, value: f64) {
        self.0.insert(field.name().to_string(), value.to_string());
    }

    fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
        self.0.insert(field.name().to_string(), value.to_string());
    }

    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        self.0.insert(field.name().to_string(), value.to_string());
    }

    fn record_bool(&mut self, field: &tracing::field::Field, value: bool) {
        self.0.insert(field.name().to_string(), value.to_string());
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        self.0.insert(field.name().to_string(), value.to_string());
    }

    fn record_error(
        &mut self,
        field: &tracing::field::Field,
        value: &(dyn std::error::Error + 'static),
    ) {
        self.0.insert(field.name().to_string(), value.to_string());
    }

    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        self.0
            .insert(field.name().to_string(), format!("{:?}", value));
    }
}

pub struct WritingFieldVisitor<'a, Writer: Write> {
    is_first: bool,
    writer: &'a mut Writer,
}

impl<'a, Writer: Write> WritingFieldVisitor<'a, Writer> {
    #[allow(unused)]
    pub fn new(writer: &'a mut Writer) -> Self {
        Self {
            is_first: false,
            writer,
        }
    }

    fn write_separator(&mut self) {
        if self.is_first {
            self.is_first = false;
        } else {
            write!(self.writer, ", ").expect("failed to write separator");
        }
    }
}

impl<'a, Writer: Write> tracing::field::Visit for WritingFieldVisitor<'a, Writer> {
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

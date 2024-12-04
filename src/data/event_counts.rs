// Copyright 2024 Ulvetanna Inc.

use super::WritingFieldVisitor;
use linear_map::LinearMap;
use std::fmt::Write;
use std::mem::take;
use std::{borrow::Cow, ops::AddAssign};

/// Number of events met during the span's lifetime.
#[derive(Default, Debug, Clone)]
pub(crate) struct EventCounts {
    events: LinearMap<Cow<'static, str>, usize>,
    buffer: String,
}

impl EventCounts {
    /// Record a new event.
    pub fn record(&mut self, event: &tracing::Event<'_>) {
        if !event.fields().any(|_| true) {
            // If no fields we can just use the event name as a key.
            let name = Cow::Borrowed(event.metadata().name());
            *self.events.entry(name).or_insert(0) += 1;
        } else {
            // If events are generating frequently in most of the cases we will be incrementing the counter
            // for already allocated string key. So, we can reuse the buffer and avoid reallocation.
            self.buffer.clear();
            write!(&mut self.buffer, "{} {{ ", event.metadata().name()).unwrap();
            let mut visitor = WritingFieldVisitor::new(&mut self.buffer);
            event.record(&mut visitor);
            write!(&mut self.buffer, " }}").unwrap();

            let key = Cow::Owned(take(&mut self.buffer));
            *self.events.entry(key).or_insert(0) += 1;
        };
    }

    pub fn increment_counter<'a>(&mut self, name: &'a str) {
        match self.events.get_mut(name) {
            Some(value) => *value += 1,
            None => {
                self.events.insert(name.to_string().into(), 1);
            }
        }
    }

    /// Check if there are no events recorded.
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Format the event counts as a string with the given separator.
    pub fn format(&self, separator: &str) -> String {
        let mut ordered_events: Vec<_> = self.events.iter().collect();
        ordered_events.sort_by_key(|(name, _)| *name);

        ordered_events
            .iter()
            .map(|(name, count)| format!("{name}: {count}"))
            .collect::<Vec<_>>()
            .join(separator)
    }
}

impl AddAssign<&EventCounts> for EventCounts {
    fn add_assign(&mut self, rhs: &EventCounts) {
        for (name, count) in &rhs.events {
            match self.events.get_mut(name) {
                Some(value) => *value += count,
                None => {
                    self.events.insert(name.clone(), *count);
                }
            }
        }
    }
}

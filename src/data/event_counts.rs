// Copyright 2024 Ulvetanna Inc.

use super::WritingFieldVisitor;
use std::fmt::Write;
use std::mem::take;
use std::{borrow::Cow, collections::HashMap, ops::AddAssign};

/// Number of events met during the span's lifetime.
#[derive(Default, Debug, Clone)]
pub struct EventCounts {
    events: HashMap<Cow<'static, str>, usize>,
    buffer: String,
}

impl EventCounts {
    /// Record a new event.
    pub fn record(&mut self, event: &tracing::Event<'_>) {
        if !event.fields().any(|_| true) {
            // If no fields we can just use the event name as a key.
            *self
                .events
                .entry(Cow::Borrowed(event.metadata().name()))
                .or_default() += 1;
        } else {
            // If events are generating frequently in most of the cases we will be incrementing the counter
            // for already allocated string key. So, we can reuse the buffer and avoid reallocation.
            self.buffer.clear();
            write!(&mut self.buffer, "{} {{ ", event.metadata().name()).unwrap();
            let mut visitor = WritingFieldVisitor::new(&mut self.buffer);
            event.record(&mut visitor);
            write!(&mut self.buffer, " }}").unwrap();

            let key = Cow::Owned(take(&mut self.buffer));
            if let Some(count) = self.events.get_mut(&key) {
                *count += 1;
                self.buffer = key.into_owned();
            } else {
                self.events.insert(key, 1);
            }
        };
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
            *self.events.entry(name.clone()).or_default() += count;
        }
    }
}

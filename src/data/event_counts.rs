// Copyright 2024-2025 Irreducible Inc.

use crate::errors::err_msg;

use super::field_visitor::{CounterValue, CounterVisitor};
use super::WritingFieldVisitor;
use linear_map::LinearMap;
use std::fmt::Write;
use std::mem::take;
use std::{borrow::Cow, ops::AddAssign};

/// Number of events met during the span's lifetime.
#[derive(Default, Debug, Clone)]
pub(crate) struct EventCounts {
    counters: LinearMap<Cow<'static, str>, CounterValue>,
    buffer: String,
}

impl EventCounts {
    /// Record a new event.
    pub fn record(&mut self, event: &tracing::Event<'_>) {
        if !event.fields().any(|_| true) {
            // If no fields we can just use the event name as a key.
            let name = Cow::Borrowed(event.metadata().name());
            match self.counters.get_mut(&name) {
                Some(value) => *value += 1,
                None => {
                    self.counters.insert(name, CounterValue::Int(1));
                }
            }
        } else {
            let mut data = CounterVisitor::default();
            event.record(&mut data);

            if data.is_counter {
                match (self.counters.get_mut(event.metadata().name()), data.value) {
                    (None, Some(new_value)) => {
                        let name = Cow::Borrowed(event.metadata().name());
                        self.counters.insert(name, new_value);
                    }
                    (Some(value), Some(new_value)) => *value += new_value,
                    _ => {
                        err_msg!("invalid event {:?}", event);
                    }
                };
            } else {
                // If events are generating frequently in most of the cases we will be incrementing the counter
                // for already allocated string key. So, we can reuse the buffer and avoid reallocation.
                self.buffer.clear();
                write!(&mut self.buffer, "{} {{ ", event.metadata().name()).unwrap();
                let mut visitor = WritingFieldVisitor::new(&mut self.buffer);
                event.record(&mut visitor);
                write!(&mut self.buffer, " }}").unwrap();

                let key = Cow::Owned(take(&mut self.buffer));
                match self.counters.get_mut(&key) {
                    Some(value) => *value += 1,
                    None => {
                        self.counters.insert(key, CounterValue::Int(1));
                    }
                }
            }
        };
    }

    pub fn increment_events_counter(&mut self, name: &str) {
        match self.counters.get_mut(name) {
            Some(value) => *value += 1,
            None => {
                self.counters
                    .insert(name.to_string().into(), CounterValue::Int(1));
            }
        }
    }

    /// Check if there are no counters recorded.
    pub fn is_empty(&self) -> bool {
        self.counters.is_empty()
    }

    /// Clear all recorded events.
    pub fn clear(&mut self) {
        self.counters.clear();
    }

    /// Format the event counts as a strings
    pub fn format(&self) -> Vec<String> {
        let mut ordered_events: Vec<_> = self.counters.iter().collect();
        ordered_events.sort_by_key(|(name, _)| *name);

        ordered_events
            .iter()
            .map(|(name, count)| format!("{name}: {count}"))
            .collect::<Vec<_>>()
    }

    #[cfg(test)]
    pub fn get(&self, key: &str) -> Option<&CounterValue> {
        self.counters.get(key)
    }
}

impl AddAssign<&EventCounts> for EventCounts {
    fn add_assign(&mut self, rhs: &EventCounts) {
        for (name, count) in &rhs.counters {
            match self.counters.get_mut(name) {
                Some(value) => *value += *count,
                None => {
                    self.counters.insert(name.clone(), *count);
                }
            };
        }
    }
}

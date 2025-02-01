// Copyright 2024 Ulvetanna Inc.

use std::fmt::Write;

pub struct LogTree {
    pub label: String,
    pub events: Vec<String>,
    pub children: Vec<LogTree>,
}

impl std::fmt::Display for LogTree {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{}", self.label)?;
        for event in &self.events {
            writeln!(f, "├>{}", event)?;
        }
        self.display_children(f, Vec::new())
    }
}

impl LogTree {
    fn display_children(&self, f: &mut std::fmt::Formatter, spaces: Vec<bool>) -> std::fmt::Result {
        for (i, child) in self.children.iter().enumerate() {
            let mut prefix = String::new();
            for is_space in &spaces {
                if *is_space {
                    write!(&mut prefix, "   ")?;
                } else {
                    write!(&mut prefix, "│  ")?;
                }
            }

            let is_last = i == self.children.len() - 1;
            // Split label to format a multiline label correctly
            let labels = child.label.split('\n');
            for (index, label) in labels.enumerate() {
                match (index == 0, is_last) {
                    (true, true) => writeln!(f, "{}└── {}", prefix, label)?,
                    (true, false) => writeln!(f, "{}├── {}", prefix, label)?,
                    (false, true) => writeln!(f, "{}    {}", prefix, label)?,
                    (false, false) => writeln!(f, "{}│   {}", prefix, label)?,
                }
            }

            for event in &child.events {
                if is_last {
                    writeln!(f, "{}   ├>{}",prefix, event)?;
                } else {
                    writeln!(f, "{}│  ├>{}",prefix, event)?;
                }
               
            }

            if !child.children.is_empty() {
                let mut next_spaces = spaces.clone();
                next_spaces.push(is_last);
                child.display_children(f, next_spaces)?;
            }
        }
        Ok(())
    }
}

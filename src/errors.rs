// Copyright 2024-2025 Irreducible Inc.

// use this instead of eprintln!
macro_rules! err_msg {
    ($($arg:tt)*) => {{
        eprintln!($($arg)*);
        assert!(cfg!(not(feature = "panic")))
    }};
}

pub(crate) use err_msg;

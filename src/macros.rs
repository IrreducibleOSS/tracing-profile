// Copyright 2024-2025 Irreducible Inc.

/// A macro that mirrors `init_tracing_with_metadata(&[(&str, String)])`
/// but lets callers write `str: "key" = "value"`, `u64: "key" = 42`, etc.
/// Under the hood, it converts values to strings.
#[macro_export]
macro_rules! init_tracing_with_metadata {
    // 1) No pairs → just call the function with an empty slice.
    () => {
        $crate::init_tracing_with_metadata(&[])
    };

    // 2) One or more `key = value` entries, comma‐separated.
    //    We capture each `key` as an identifier and `value` as any expression.
    (
        $(
            $key:ident = $val:expr
        ),* $(,)?
    ) => {
        $crate::init_tracing_with_metadata(&[
            $(
                // Turn `key` into a string at compile‐time, and
                // call `.to_string()` on every `value`.
                (
                    stringify!($key),
                    $val.to_string(),
                )
            ),*
        ])
    };
}

/// A helper struct to wrap a guard and an inner object.
#[allow(unused)]

pub struct GuardWrapper<Current, Inner>(Current, Inner);

impl<Current, Inner> GuardWrapper<Current, Inner> {
    /// Create a new `GuardWrapper` with the given `current` and `inner` object.
    #[allow(unused)]
    pub fn wrap(current: Current, inner: Inner) -> Self {
        Self(current, inner)
    }
}

/// We use `GuardWrapper` as return type for `init_tracing` function in `layers/init_tracing.rs`
/// as a transparent `impl Drop` object. That's why we need to implement `Drop` trait for it.
impl<Current, Inner> Drop for GuardWrapper<Current, Inner> {
    fn drop(&mut self) {}
}

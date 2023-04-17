/// Type to represent *'no software controlled NSS'*.
pub struct DummyNSS;

/// Type to represent *'no delay function'*.
pub struct DummyDelay;

mod sealed {
    /// A trait that can be implemented to limit implementations to this crate.
    /// See the [Sealed traits pattern](https://rust-lang.github.io/api-guidelines/future-proofing.html)
    /// for more info.
    pub trait Sealed {}
}

pub(crate) use sealed::Sealed;

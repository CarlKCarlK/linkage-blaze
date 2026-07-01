//! Helper for consuming `Result<T, Infallible>` values without `.expect(...)`.

use core::convert::Infallible;

/// Extension for unwrapping a [`Result`] whose error is [`Infallible`].
pub trait InfallibleResultExt<T> {
    /// Unwraps the `Ok` value; the error type is [`Infallible`], so this
    /// compiles away to nothing.
    fn unwrap_infallible(self) -> T;
}

impl<T> InfallibleResultExt<T> for Result<T, Infallible> {
    fn unwrap_infallible(self) -> T {
        self.unwrap_or_else(|never| match never {})
    }
}

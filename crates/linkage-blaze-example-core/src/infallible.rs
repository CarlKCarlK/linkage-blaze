//! Helper for consuming `Result<T, Infallible>` values without `.expect(...)`.

use core::convert::Infallible;

/// Extension for unwrapping a [`Result`] whose error is [`Infallible`].
pub trait InfallibleResultExt<T> {
    /// Unwraps the `Ok` value; the error case is the never case, so this
    /// compiles away to nothing.
    fn unwrap_never(self) -> T;
}

impl<T> InfallibleResultExt<T> for Result<T, Infallible> {
    fn unwrap_never(self) -> T {
        self.unwrap_or_else(|never| match never {})
    }
}

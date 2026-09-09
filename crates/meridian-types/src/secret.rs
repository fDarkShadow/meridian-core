use std::fmt;

pub struct Secret<T>(T);

impl<T> Secret<T> {
    pub fn new(value: T) -> Self {
        Self(value)
    }

    // Sole unwrap path, pending the host I/O capability (spec/03-node-protocol.md) that
    // should gate it instead once it exists.
    pub fn expose_secret(&self) -> &T {
        &self.0
    }

    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Secret<U> {
        Secret(f(self.0))
    }
}

impl<T> fmt::Debug for Secret<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[REDACTED]")
    }
}

impl<T> fmt::Display for Secret<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[REDACTED]")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_and_display_redact_the_value() {
        let secret = Secret::new("super-secret-token".to_string());
        assert_eq!(format!("{secret:?}"), "[REDACTED]");
        assert_eq!(format!("{secret}"), "[REDACTED]");
    }

    #[test]
    fn debug_and_display_redact_regardless_of_inner_type() {
        let secret = Secret::new(42_i64);
        assert_eq!(format!("{secret:?}"), "[REDACTED]");
        assert_eq!(format!("{secret}"), "[REDACTED]");
    }

    #[test]
    fn taint_propagates_to_derived_values() {
        let secret = Secret::new("token".to_string());
        let derived: Secret<usize> = secret.map(|s| s.len());

        assert_eq!(format!("{derived:?}"), "[REDACTED]");
        assert_eq!(format!("{derived}"), "[REDACTED]");
        assert_eq!(*derived.expose_secret(), 5);
    }

    #[test]
    fn expose_secret_returns_the_wrapped_value() {
        let secret = Secret::new(vec![1, 2, 3]);
        assert_eq!(secret.expose_secret(), &vec![1, 2, 3]);
    }
}

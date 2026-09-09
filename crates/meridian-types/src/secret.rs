//! The `Secret<T>` taint-tracked type (`INV-007`, `DAT-012`).
//!
//! Wraps a value so it can never be observed through `Debug`/`Display`, and so that values
//! derived from it stay wrapped too, per `INV-007`'s taint-propagation requirement.
//!
//! `expose_secret` is the one explicit, greppable way to reach the inner value. It exists
//! because no host-mediated I/O capability exists yet to gate it. Once the capability
//! described in `spec/03-node-protocol.md` (`PROTO-003`) and `SEC-007` exists, access MUST be
//! routed through that capability instead of this method's current public visibility. This
//! type does not by itself implement envelope encryption, `SecretProvider`, or egress-scope —
//! see `spec/04-secrets-security.md` for those.

use std::fmt;

/// A value that must never appear in a log, an output, a model context, or the IR in plain
/// form (`INV-007`).
pub struct Secret<T>(T);

impl<T> Secret<T> {
    /// Wraps `value`, tainting it.
    pub fn new(value: T) -> Self {
        Self(value)
    }

    /// The one explicit path to the inner value. See the module docs for why this exists and
    /// what it is expected to become.
    pub fn expose_secret(&self) -> &T {
        &self.0
    }

    /// Derives a new value from the secret while keeping the result tainted, so transforming a
    /// secret does not accidentally launder it into a plain value (`INV-007`: "taint MUST
    /// propagate to derived values").
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

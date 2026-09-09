//! Placeholder crate. Real modules land as their `spec/` requirement issues are implemented
//! (see `spec/CONVENTIONS.md` and `spec/11-build-order.md`).

/// Returns the crate version from `Cargo.toml`, used as a smoke-test wired into
/// CI and the container entrypoint until real functionality lands.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_not_empty() {
        assert!(!version().is_empty());
    }
}

//! Foundational types shared across the engine (`spec/02-data-model.md`).
//!
//! This crate holds types that every other crate — IR, node protocol, secret storage — needs
//! to agree on from day one, per `DAT-012`: "`Secret<T>` MUST be a distinct, taint-tracked
//! type that is part of the base type system through which all engine data circulates."

mod secret;

pub use secret::Secret;

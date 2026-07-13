//! Domain types and application use cases for Verba.

pub mod capture;
pub mod presentation;
pub mod shortcut;

#[cfg(any(test, feature = "test-support"))]
pub mod testing;

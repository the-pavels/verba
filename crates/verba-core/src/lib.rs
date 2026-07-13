//! Domain types and application use cases for Verba.

pub mod capture;
pub mod coordinator;
pub mod presentation;
pub mod shortcut;
pub mod translation;

#[cfg(any(test, feature = "test-support"))]
pub mod testing;

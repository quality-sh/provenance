//! The build-time rejection type.

use std::fmt;

/// Every violation `build()` found, in evaluation order.
#[derive(Debug)]
pub struct AuthoringError {
    violations: Vec<String>,
}

impl AuthoringError {
    pub(super) const fn new(violations: Vec<String>) -> Self {
        Self { violations }
    }

    pub fn violations(&self) -> &[String] {
        &self.violations
    }
}

impl fmt::Display for AuthoringError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.violations.join("\n"))
    }
}

impl std::error::Error for AuthoringError {}

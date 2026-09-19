//! Shared human-facing semantics for Provenance interfaces.

pub mod check;
pub mod get;

/// A shared Porcelain capability selected by an interface binding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Action {
    /// Read one known record.
    Get,
    /// Check repository obligations.
    Check,
}

/// A semantic request for an action on one known record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordRequest {
    /// The repository-local record ID.
    pub target: String,
    /// The action to apply.
    pub action: Action,
}

impl RecordRequest {
    /// Create a semantic record request.
    pub fn new(target: impl Into<String>, action: Action) -> Self {
        Self {
            target: target.into(),
            action,
        }
    }
}

/// A semantic result with readable and typed forms.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Outcome<T> {
    /// A concise readable result.
    pub summary: String,
    /// The typed result data.
    pub data: T,
}

impl<T> Outcome<T> {
    /// Create an outcome for surface-specific rendering.
    pub fn new(summary: impl Into<String>, data: T) -> Self {
        Self {
            summary: summary.into(),
            data,
        }
    }
}

/// Shared Porcelain capabilities over a caller-supplied operation port.
#[derive(Clone, Debug)]
pub struct Porcelain<P> {
    pub(crate) port: P,
}

impl<P> Porcelain<P> {
    /// Create Porcelain capabilities with an injected operation port.
    pub const fn new(port: P) -> Self {
        Self { port }
    }

    /// Return the injected port.
    pub fn into_port(self) -> P {
        self.port
    }
}

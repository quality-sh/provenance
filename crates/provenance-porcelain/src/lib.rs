//! Shared human-facing semantics for Provenance interfaces.
//!
//! Get and check request and result types produce their JSON and MCP schemas here.
//! Add a get result field to the typed projection in `get::wire`.
//! Add an action to `action::Action`, then register its operations in the catalog.

pub mod check;
pub mod action;
pub mod get;
pub mod search;

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

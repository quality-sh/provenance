//! This crate is an implementation part of Provenance. Provenance is pre-release, and this API can
//! change without compatibility shims. Use `provenance-sdk` for Rust.
//!
//! Shared human-facing semantics for Provenance interfaces.
//!
//! Get and check request and result types produce their JSON schemas here.
//! Add a get result field to the typed projection in `get::wire`.
//! Record actions use catalog registrations. Discussion actions use typed inputs.
//! Materialize, import, dictionary import, and swarm-backtrace land run through their
//! own ports; each port reports its own failure type.

pub mod action;
pub mod action_input;
pub mod api;
pub mod cargo_init;
pub mod check;
pub mod dictionary;
pub mod discussion;
pub mod export;
pub mod get;
pub mod graph_reference;
pub mod guidance;
pub mod import;
pub mod init;
pub mod materialize;
pub mod registry;
pub mod report;
pub mod schema;
pub mod search;
pub mod swarm_backtrace;
pub mod validate;

/// The future of one port call: the output, or the failure that the port reports.
pub type WorkFuture<'a, T, F> =
    std::pin::Pin<Box<dyn std::future::Future<Output = Result<T, F>> + Send + 'a>>;

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

/// The JSON Schema of one result type, as the result serializes.
pub(crate) fn result_schema<T: schemars::JsonSchema>() -> serde_json::Value {
    serde_json::to_value(
        schemars::generate::SchemaSettings::draft2020_12()
            .with(|settings| settings.contract = schemars::generate::Contract::Serialize)
            .into_generator()
            .into_root_schema_for::<T>(),
    )
    .expect("a result schema is JSON")
}

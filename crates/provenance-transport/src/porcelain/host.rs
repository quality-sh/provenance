//! Porcelain construction for one statement host.

use super::{HostApiPort, HostDiscussionPort, HostGetPort, HostSearchPort};
use crate::StatementHost;
use provenance_porcelain::check::{Category, CheckPort, PortFuture};
use provenance_porcelain::Porcelain;
use std::sync::Arc;

/// Constructs Porcelain capabilities with the matching host adapter.
pub struct HostPorcelain<'a> {
    pub(super) host: &'a StatementHost,
}

impl<'a> HostPorcelain<'a> {
    pub(crate) const fn new(host: &'a StatementHost) -> Self {
        Self { host }
    }

    pub fn api(&self) -> Porcelain<HostApiPort> {
        Porcelain::new(HostApiPort::new(self.host.clone()))
    }

    /// The check command with this host's check adapter. A host without one reports
    /// each category as unavailable.
    pub fn check(&self) -> Porcelain<Arc<dyn CheckPort>> {
        let port = self
            .host
            .check_port()
            .cloned()
            .unwrap_or_else(|| Arc::new(NoRepository));
        Porcelain::new(port)
    }

    pub fn discussion(&self) -> Porcelain<HostDiscussionPort> {
        Porcelain::new(HostDiscussionPort::new(self.host.clone()))
    }

    pub fn get(&self) -> Porcelain<HostGetPort> {
        Porcelain::new(HostGetPort::new(self.host.clone()))
    }

    pub fn search(&self) -> Porcelain<HostSearchPort> {
        Porcelain::new(HostSearchPort::new(self.host.clone()))
    }
}

/// The check adapter of a host that has no repository to check.
struct NoRepository;

impl CheckPort for NoRepository {
    fn run<'a>(&'a self, _category: Category, _scope: Option<&'a str>) -> PortFuture<'a> {
        Box::pin(async { Err("this host has no repository to check".to_owned()) })
    }
}

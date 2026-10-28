//! Porcelain construction for one statement host.

use super::{HostApiPort, HostDiscussionPort, HostGetPort, HostSearchPort};
use crate::StatementHost;
use provenance_porcelain::check::CheckPort;
use provenance_porcelain::Porcelain;
use std::sync::Arc;

/// Constructs Porcelain capabilities with the matching host adapter.
pub struct HostPorcelain<'a> {
    host: &'a StatementHost,
}

impl<'a> HostPorcelain<'a> {
    pub(crate) const fn new(host: &'a StatementHost) -> Self {
        Self { host }
    }

    pub fn api(&self) -> Porcelain<HostApiPort> {
        Porcelain::new(HostApiPort::new(self.host.clone()))
    }

    pub fn check(&self, port: Arc<dyn CheckPort>) -> Porcelain<Arc<dyn CheckPort>> {
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

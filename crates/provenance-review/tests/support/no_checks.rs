//! A check port for a host whose test runs no check.
use provenance_porcelain::check::{Category, CheckPort, PortFuture};

/// A check port for a host whose test never runs a check.
pub struct NoChecks;

impl CheckPort for NoChecks {
    fn run<'a>(&'a self, _: Category, _: Option<&'a str>) -> PortFuture<'a> {
        Box::pin(async { Err("this host runs no checks".to_owned()) })
    }
}

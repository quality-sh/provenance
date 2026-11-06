//! The materialize adapter: the query cache of one repository, rebuilt from its files.

use super::RepositoryPort;
use provenance_porcelain::materialize::{MaterializeInput, MaterializeOutput, MaterializePort};
use provenance_porcelain::WorkFuture;

impl MaterializePort for RepositoryPort {
    type Failure = anyhow::Error;

    fn materialize(
        &self,
        _input: MaterializeInput,
    ) -> WorkFuture<'_, MaterializeOutput, anyhow::Error> {
        Box::pin(async move {
            let repository = self.context.open()?;
            let report = provenance_store::cache::materialize_state(&repository).await?;
            Ok(MaterializeOutput {
                records_loaded: report.records_loaded,
                cache_recreated: report.cache_recreated,
            })
        })
    }
}

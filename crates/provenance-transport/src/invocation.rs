//! The one host invocation. Every store caller passes through it.
use crate::StatementHost;
use provenance_core::protocol::{
    failure::{ErasedFailure, OperationFailure},
    repository::{RepositoryContext, RepositoryScope, RepositoryTarget},
};
use provenance_macros::rule;
use provenance_store::operations::catalog::{
    self, ContextKind, ExecutionNeeds, Operation, PreparedContext, RequestedContext,
};
use provenance_store::operations::read_operations::{self, ReadOperations};
use serde_json::Value;
use std::sync::Arc;

/// How one call enters the host: its context, its framing, and its page-write mark.
pub struct Invocation {
    selected: Option<RequestedContext>,
    framing: Framing,
    page_write: bool,
}

/// Where a query page meets its bound.
enum Framing {
    /// The store bounds the page and the typed result goes back as it is.
    Native,
    /// The routed response adapter bounds the encoded envelope and adds the freshness cause.
    Wire,
}

impl Invocation {
    /// A typed caller: the bound identity, native framing, the page bound inside the store.
    #[must_use]
    pub const fn native() -> Self {
        Self {
            selected: None,
            framing: Framing::Native,
            page_write: false,
        }
    }

    /// The routing adapter only: the decoded wire context, external framing.
    pub(crate) const fn wire(selected: Option<RequestedContext>, page_write: bool) -> Self {
        Self {
            selected,
            framing: Framing::Wire,
            page_write,
        }
    }
}

impl StatementHost {
    /// Runs one store operation on this host. Advertisement, request validation and the
    /// identity check answer on the runtime thread before the work slot wait; repository
    /// preparation and the operation run on the tracked blocking pool. For a marked call,
    /// preparation runs first and the operation then runs in the page-write section, through
    /// the repository of that section.
    #[rule("rule_lock_wait_does_not_delay_host_calls")]
    #[rule("rule_host_refusal_before_slot_wait")]
    pub async fn invoke<O: Operation>(
        &self,
        request: O::Request,
        options: Invocation,
    ) -> Result<O::Success, O::Failure> {
        // A wire call is advertised by its public route in the routing adapter.
        if matches!(options.framing, Framing::Native) && !self.advertises(O::NAME) {
            return Err(OperationFailure::AccessDenied.into());
        }
        let selected = match options.selected {
            Some(selected) => Some(selected),
            None => self.bound_context(O::CONTEXT)?,
        };
        O::validate_external(&request)?;
        let needs = O::needs(&request);
        let page_writes = options
            .page_write
            .then(|| self.page_writes.clone())
            .flatten();
        let runtime = tokio::runtime::Handle::current();
        let native = matches!(options.framing, Framing::Native);
        self.admitted(
            O::NAME,
            selected,
            needs,
            native,
            move |context| match page_writes {
                Some(page) => page
                    .run(|held| {
                        let context = context.within(held)?;
                        runtime.block_on(catalog::invoke_typed::<O>(context, request))
                    })
                    .unwrap_or_else(|| Err(OperationFailure::Internal.into())),
                None => runtime.block_on(catalog::invoke_typed::<O>(context, request)),
            },
        )
        .await?
    }

    /// Authorizes `selected` on the runtime thread, waits for a work slot, then prepares the
    /// repository and runs `work` on the tracked blocking pool. Each refusal answers before
    /// `work` runs.
    async fn admitted<T: Send + 'static>(
        &self,
        operation: &'static str,
        selected: Option<RequestedContext>,
        needs: ExecutionNeeds,
        native: bool,
        work: impl FnOnce(PreparedContext) -> T + Send + 'static,
    ) -> Result<T, OperationFailure> {
        if let Some(selected) = &selected {
            self.access.authorize(operation, selected)?;
        }
        let access = self.access.clone();
        self.execution
            .run(move || prepare(&access, operation, selected, needs, native).map(work))
            .await?
    }

    /// The read operations port of the bound repository, when `operation` is one of its
    /// operations.
    pub(crate) fn read_port(&self, operation: &str) -> Option<Arc<dyn ReadOperations>> {
        read_operations::read_operation(operation)?;
        self.access.read_operations()
    }

    /// Runs one read operation through `port`. The call has the checks, the authorization,
    /// the work slot and the repository preparation of an operation that this host runs, in
    /// the same order; then the port runs on the tracked blocking pool, and the dispatch checks
    /// its answer against the operation.
    pub(crate) async fn read(
        &self,
        port: Arc<dyn ReadOperations>,
        operation: &str,
        call: Value,
    ) -> Result<Value, ErasedFailure> {
        let runtime = tokio::runtime::Handle::current();
        provenance_contract::invoke_port(operation, call, |operation, selected, needs, call| {
            self.admitted(operation, selected, needs, false, move |_prepared| {
                runtime.block_on(port.run(operation, call))
            })
        })
        .await
    }

    /// The bound identity in the shape the operation's context kind expects.
    fn bound_context(
        &self,
        kind: ContextKind,
    ) -> Result<Option<RequestedContext>, OperationFailure> {
        let identity = || {
            self.bound_identity()
                .ok_or(OperationFailure::UnavailableNeeds)
        };
        Ok(match kind {
            ContextKind::DataFree => None,
            ContextKind::Repository => {
                let (repository, _) = identity()?;
                Some(RequestedContext::Repository(RepositoryTarget {
                    repository,
                }))
            }
            ContextKind::Scope => {
                let (repository, scope) = identity()?;
                Some(RequestedContext::Scope(RepositoryScope {
                    repository,
                    scope,
                }))
            }
            ContextKind::Scoped => {
                let (repository, scope) = identity()?;
                Some(RequestedContext::Scoped(RepositoryContext {
                    repository,
                    scope,
                    freshness: None,
                }))
            }
        })
    }
}

/// Repository preparation, on the blocking path before any page-write section.
fn prepare(
    access: &Arc<dyn crate::HostAccess>,
    operation: &'static str,
    selected: Option<RequestedContext>,
    needs: ExecutionNeeds,
    native: bool,
) -> Result<PreparedContext, OperationFailure> {
    let context = match selected {
        Some(selected) => access.prepare(operation, selected, needs)?,
        None => PreparedContext::data_free(),
    };
    Ok(if native {
        context.into_native()
    } else {
        context
    })
}

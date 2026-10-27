//! The one host invocation. Every store caller passes through it.
use crate::StatementHost;
use provenance_core::protocol::{
    failure::{OperationError, OperationFailure},
    repository::{RepositoryContext, RepositoryScope, RepositoryTarget},
};
use provenance_macros::rule;
use provenance_store::operations::catalog::{
    self, ContextKind, Operation, PreparedContext, RequestedContext,
};
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
    #[allow(dead_code)]
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
    /// preparation and the operation run on the tracked blocking pool, inside the page-write
    /// observer when the call is marked.
    #[rule("rule_lock_wait_does_not_delay_host_calls")]
    #[rule("rule_host_refusal_before_slot_wait")]
    pub async fn invoke<O: Operation>(
        &self,
        request: O::Request,
        options: Invocation,
    ) -> Result<O::Success, OperationError<O::Failure>> {
        if !self.advertises(O::NAME) {
            return Err(OperationError::Common(OperationFailure::AccessDenied));
        }
        let selected = match options.selected {
            Some(selected) => Some(selected),
            None => self.bound_context(O::CONTEXT)?,
        };
        O::validate_external(&request).map_err(OperationError::Common)?;
        if let Some(selected) = &selected {
            self.access
                .authorize(O::NAME, selected)
                .map_err(OperationError::Common)?;
        }
        let access = self.access.clone();
        let page_writes = options
            .page_write
            .then(|| self.page_writes.clone())
            .flatten();
        let runtime = tokio::runtime::Handle::current();
        let native = matches!(options.framing, Framing::Native);
        self.execution
            .run(move || {
                let work = || runtime.block_on(prepared::<O>(&access, selected, request, native));
                match page_writes {
                    Some(page) => page.run(work).unwrap_or_else(|_| {
                        Err(OperationError::Common(OperationFailure::Internal))
                    }),
                    None => work(),
                }
            })
            .await
            .map_err(OperationError::Common)?
    }

    /// The bound identity in the shape the operation's context kind expects.
    fn bound_context<F>(
        &self,
        kind: ContextKind,
    ) -> Result<Option<RequestedContext>, OperationError<F>> {
        let identity = || {
            self.bound_identity()
                .ok_or(OperationError::Common(OperationFailure::UnavailableNeeds))
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

/// Repository preparation and the operation, on the blocking path.
async fn prepared<O: Operation>(
    access: &Arc<dyn crate::HostAccess>,
    selected: Option<RequestedContext>,
    request: O::Request,
    native: bool,
) -> Result<O::Success, OperationError<O::Failure>> {
    let context = match selected {
        Some(selected) => access
            .prepare(O::NAME, selected, O::needs(&request))
            .map_err(OperationError::Common)?,
        None => PreparedContext::data_free(),
    };
    let context = if native {
        context.into_native()
    } else {
        context
    };
    catalog::invoke_typed::<O>(context, request).await
}

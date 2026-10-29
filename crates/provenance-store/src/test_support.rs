#[path = "../tests/support/initialization.rs"]
mod initialization;
pub use initialization::allow_reviewer;

/// The fixture repository at `layout`, opened for `default`, or for the
/// first scope its manifest declares. The open runs publication recovery
/// before it reads the manifest.
pub fn repository(layout: &crate::layout::ProvenanceLayout) -> crate::repository::Repository {
    use crate::repository::{OpenFailure, Repository};
    match Repository::open(layout.root(), "default") {
        Ok(repository) => repository,
        Err(OpenFailure::UnknownScope { declared, .. }) => {
            Repository::open(layout.root(), &declared[0]).unwrap()
        }
        Err(error) => panic!("open the fixture repository {}: {error}", layout.root()),
    }
}

/// A store over the fixture repository at `layout`.
pub fn store(layout: &crate::layout::ProvenanceLayout) -> crate::state_store::StateStore {
    repository(layout).store()
}

/// Runs operation `O` on `host` from its JSON call, `{"context": …, "request": …}`, without
/// lookup by name. A data-free operation has no context. A refusal is framed as the
/// operation declares it.
#[allow(clippy::result_large_err)]
pub async fn call<O, H>(
    call: serde_json::Value,
    host: H,
) -> Result<serde_json::Value, provenance_core::protocol::failure::ErasedFailure>
where
    O: crate::operations::catalog::Operation,
    H: crate::operations::catalog::Invoke,
{
    use crate::operations::catalog::{frame_failure, ContextKind, RequestedContext};
    fn decode<T: serde::de::DeserializeOwned>(value: &serde_json::Value, part: &str) -> T {
        serde_json::from_value(value.clone())
            .unwrap_or_else(|error| panic!("decode the {part}: {error}"))
    }
    let context = &call["context"];
    let selected = match O::CONTEXT {
        ContextKind::Repository => Some(RequestedContext::Repository(decode(context, "context"))),
        ContextKind::Scope => Some(RequestedContext::Scope(decode(context, "context"))),
        ContextKind::Scoped => Some(RequestedContext::Scoped(decode(context, "context"))),
        ContextKind::DataFree => None,
    };
    let request: O::Request = decode(&call["request"], "request");
    let success = host
        .invoke::<O>(selected, request)
        .await
        .map_err(frame_failure::<O>)?;
    Ok(serde_json::to_value(success).expect("encode the success"))
}

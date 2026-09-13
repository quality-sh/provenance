//! Native updates use the typed catalog without an HTTP host.
use crate::{cli::updates::UpdateArgs, output};
use provenance_core::ScopeId;
use provenance_store::{operations::catalog::Operation, write_error::WriteError};

pub(super) async fn handle<O: Operation<Failure = WriteError>>(
    args: UpdateArgs,
) -> anyhow::Result<()> {
    let mut fields: serde_json::Map<String, serde_json::Value> =
        super::common::parse_json_arg("fields-json", &args.fields_json)?;
    anyhow::ensure!(!fields.is_empty(), "at least one field must be updated");
    anyhow::ensure!(
        !fields.contains_key("scope_id") && !fields.contains_key("id"),
        "fields-json must not contain scope_id or id; use the command flags"
    );
    fields.insert("scope_id".into(), args.scope.clone().into());
    fields.insert("id".into(), args.id.into());
    let request = serde_json::from_value(fields.into())?;
    let result =
        super::native::invoke_native::<O>(args.repo, ScopeId::new(args.scope)?, request).await?;
    output::print_json(&result)
}
